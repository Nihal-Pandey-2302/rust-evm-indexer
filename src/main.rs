// src/main.rs
mod api;
mod api_models;
mod db;
mod docs;
mod models;
use dotenvy::dotenv;
use ethers::{
    providers::{Http, Middleware, Provider},
    types::U64,
};
use eyre::Result;
use futures::stream::{self, StreamExt};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use models::{MyBlock, MyLog, MyTransaction};

// --- Constants for Ingester ---
const POLL_INTERVAL_SECONDS: u64 = 10;
const BLOCKS_PER_BATCH: u64 = 5;
const DEFAULT_START_BLOCK: u64 = 23900790; // Configurable via START_BLOCK env var
const MAX_RECEIPT_CONCURRENT: usize = 10; // Max parallel receipt fetches per block
const MAX_BLOCK_FETCH_RETRIES: u32 = 3;
const BASE_BLOCK_FETCH_BACKOFF_SECONDS: u64 = 2;

// Fetches a receipt with retries. Returns None if the RPC returns Ok(None).
async fn fetch_receipt_with_retry(
    provider: Arc<Provider<Http>>,
    tx_hash: ethers::types::H256,
    block_num: u64,
) -> Result<Option<ethers::types::TransactionReceipt>> {
    for attempt in 1..=3u32 {
        match provider.get_transaction_receipt(tx_hash).await {
            Ok(r) => return Ok(r),
            Err(e) => {
                if attempt == 3 {
                    return Err(eyre::eyre!(
                        "Failed to fetch receipt for tx {:?} in block {} after 3 attempts: {:?}",
                        tx_hash,
                        block_num,
                        e
                    ));
                }
                let backoff = Duration::from_secs(2_u64.pow(attempt - 1));
                warn!(
                    "Receipt fetch attempt {}/3 for {:?} failed: {}. Retrying in {}s...",
                    attempt,
                    tx_hash,
                    e,
                    backoff.as_secs()
                );
                tokio::time::sleep(backoff).await;
            }
        }
    }
    unreachable!()
}

async fn run_continuous_ingester(provider: Arc<Provider<Http>>, pool: PgPool) -> Result<()> {
    info!("--- Continuous Ingester Task Started ---");
    info!(
        "Polling every {}s, batch size {}.",
        POLL_INTERVAL_SECONDS, BLOCKS_PER_BATCH
    );

    loop {
        let last_synced_block_opt = match db::get_last_synced_block(&pool).await {
            Ok(val) => val,
            Err(e) => {
                error!(
                    "INGESTER DB: Failed to get last synced block: {}. Retrying in {}s.",
                    e, POLL_INTERVAL_SECONDS
                );
                tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
                continue;
            }
        };

        let start_block_to_fetch: u64 = match last_synced_block_opt {
            Some(last_block) => last_block + 1,
            None => {
                let start = env::var("START_BLOCK")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(DEFAULT_START_BLOCK);
                info!("No last synced block in DB. Starting from block {}.", start);
                start
            }
        };

        let current_chain_head = match provider.get_block_number().await {
            Ok(head) => head.as_u64(),
            Err(e) => {
                error!(
                    "INGESTER ETH: Failed to get chain head: {}. Retrying in {}s.",
                    e, POLL_INTERVAL_SECONDS
                );
                tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
                continue;
            }
        };

        if start_block_to_fetch > current_chain_head {
            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
            continue;
        }

        let end_block_to_fetch =
            (start_block_to_fetch + BLOCKS_PER_BATCH - 1).min(current_chain_head);

        info!(
            "INGESTER Cycle: blocks {} → {} (chain head: {})",
            start_block_to_fetch, end_block_to_fetch, current_chain_head
        );

        let mut blocks_processed_this_cycle = 0u64;

        'batch: for block_num_u64 in start_block_to_fetch..=end_block_to_fetch {
            let block_num_for_rpc = U64::from(block_num_u64);

            // --- Fetch block with retries ---
            let mut ethers_block_opt = None;
            for attempt in 1..=MAX_BLOCK_FETCH_RETRIES {
                match provider.get_block_with_txs(block_num_for_rpc).await {
                    Ok(Some(b)) => {
                        ethers_block_opt = Some(b);
                        break;
                    }
                    Ok(None) => {
                        warn!(
                            "Block #{} not found on attempt {}. Skipping.",
                            block_num_u64, attempt
                        );
                        break;
                    }
                    Err(e) => {
                        error!(
                            "Attempt {}/{} failed to fetch block #{}: {:?}.",
                            attempt, MAX_BLOCK_FETCH_RETRIES, block_num_u64, e
                        );
                        if attempt == MAX_BLOCK_FETCH_RETRIES {
                            error!(
                                "Giving up on block #{} after max retries. Stopping batch.",
                                block_num_u64
                            );
                            break 'batch;
                        }
                        let backoff = Duration::from_secs(
                            BASE_BLOCK_FETCH_BACKOFF_SECONDS * 2_u64.pow(attempt - 1),
                        );
                        tokio::time::sleep(backoff).await;
                    }
                }
            }

            let ethers_block = match ethers_block_opt {
                Some(b) => b,
                None => continue,
            };

            // --- Reorg detection: validate parent_hash ---
            if block_num_u64 > 0 {
                let stored_hash =
                    db::get_canonical_block_hash_at_height(&pool, block_num_u64 - 1).await;
                if let Ok(Some(stored)) = stored_hash {
                    let parent = format!("{:#x}", ethers_block.parent_hash);
                    if parent != stored {
                        warn!(
                            "REORG DETECTED at height {}! Expected parent {}, got {}. Rolling back from height {}.",
                            block_num_u64, stored, parent, block_num_u64 - 1
                        );
                        if let Err(e) = db::rollback_from_height(&pool, block_num_u64 - 1).await {
                            error!("Rollback failed: {}. Stopping ingester cycle.", e);
                            break 'batch;
                        }
                        info!(
                            "Rollback complete. Re-ingesting from height {}.",
                            block_num_u64 - 1
                        );
                        // Skip this block; next cycle will re-fetch from correct height
                        break 'batch;
                    }
                }
            }

            let my_block = MyBlock {
                block_number: ethers_block.number.unwrap_or_default(),
                block_hash: ethers_block.hash.unwrap_or_default(),
                parent_hash: ethers_block.parent_hash,
                timestamp: ethers_block.timestamp,
                gas_used: ethers_block.gas_used,
                gas_limit: ethers_block.gas_limit,
                base_fee_per_gas: ethers_block.base_fee_per_gas,
            };

            let transactions = ethers_block.transactions;
            let total_txs = transactions.len();

            // --- Phase 1: Parallel receipt fetching (RPC I/O only) ---
            info!(
                "Block #{}: fetching {} receipts in parallel (concurrency={})...",
                block_num_u64, total_txs, MAX_RECEIPT_CONCURRENT
            );

            let provider_arc = provider.clone();
            let tx_receipts: Vec<(
                ethers::types::Transaction,
                Option<ethers::types::TransactionReceipt>,
            )> = stream::iter(transactions)
                .map(|tx| {
                    let p = provider_arc.clone();
                    async move {
                        let hash = tx.hash;
                        let receipt = fetch_receipt_with_retry(p, hash, block_num_u64)
                            .await
                            .unwrap_or(None);
                        (tx, receipt)
                    }
                })
                .buffer_unordered(MAX_RECEIPT_CONCURRENT)
                .collect()
                .await;

            // --- Phase 2: Sequential DB writes inside a single atomic transaction ---
            let block_processing_result: Result<()> = async {
                let mut db_tx = pool
                    .begin()
                    .await
                    .map_err(|e| eyre::eyre!("DB: begin tx for block #{}: {}", block_num_u64, e))?;

                db::insert_block_data(&mut db_tx, &my_block)
                    .await
                    .map_err(|e| eyre::eyre!("DB: insert block #{}: {}", block_num_u64, e))?;

                for (idx, (ethers_tx, receipt_opt)) in tx_receipts.into_iter().enumerate() {
                    if idx % 50 == 0 || idx == total_txs - 1 {
                        info!(
                            "  Block #{}: writing tx {}/{}...",
                            block_num_u64,
                            idx + 1,
                            total_txs
                        );
                    }

                    let status = receipt_opt
                        .as_ref()
                        .and_then(|r| r.status)
                        .map(|s| s.as_u64());
                    let my_tx = MyTransaction {
                        tx_hash: ethers_tx.hash,
                        block_number: ethers_tx.block_number.unwrap_or_default(),
                        block_hash: ethers_tx.block_hash.unwrap_or_default(),
                        transaction_index: ethers_tx.transaction_index,
                        from_address: ethers_tx.from,
                        to_address: ethers_tx.to,
                        value: ethers_tx.value,
                        gas_price: ethers_tx.gas_price,
                        max_fee_per_gas: ethers_tx.max_fee_per_gas,
                        max_priority_fee_per_gas: ethers_tx.max_priority_fee_per_gas,
                        gas: ethers_tx.gas,
                        input_data: ethers_tx.input.to_string(),
                        status,
                    };

                    db::insert_transaction_data(&mut db_tx, &my_tx)
                        .await
                        .map_err(|e| eyre::eyre!("DB: insert tx {:?}: {}", my_tx.tx_hash, e))?;

                    if let Some(ref receipt) = receipt_opt {
                        for ethers_log in &receipt.logs {
                            let my_log = MyLog {
                                log_index: ethers_log.log_index,
                                transaction_hash: ethers_log.transaction_hash.unwrap_or_default(),
                                transaction_index: ethers_log.transaction_index.map(|i| i.as_u64()),
                                block_number: ethers_log.block_number.map_or(0, |bn| bn.as_u64()),
                                block_hash: ethers_log.block_hash.unwrap_or_default(),
                                address: ethers_log.address,
                                data: ethers_log.data.to_string(),
                                topics: ethers_log
                                    .topics
                                    .iter()
                                    .map(|h| format!("{:#x}", h))
                                    .collect(),
                            };
                            db::insert_log_data(&mut db_tx, &my_log)
                                .await
                                .map_err(|e| {
                                    eyre::eyre!("DB: insert log for tx {:?}: {}", my_tx.tx_hash, e)
                                })?;
                        }
                    }
                }

                db::set_last_synced_block(&mut db_tx, block_num_u64, current_chain_head)
                    .await
                    .map_err(|e| {
                        eyre::eyre!("DB: set_last_synced_block #{}: {}", block_num_u64, e)
                    })?;
                db_tx
                    .commit()
                    .await
                    .map_err(|e| eyre::eyre!("DB: commit block #{}: {}", block_num_u64, e))?;

                Ok(())
            }
            .await;

            match block_processing_result {
                Ok(()) => {
                    info!(
                        "✓ Block #{} committed (lag: {} blocks behind chain head).",
                        block_num_u64,
                        current_chain_head.saturating_sub(block_num_u64)
                    );
                    blocks_processed_this_cycle += 1;
                }
                Err(e) => {
                    error!(
                        "INGESTER: block #{} failed: {}. Rolled back. Retrying next cycle.",
                        block_num_u64, e
                    );
                    break 'batch;
                }
            }
        }

        info!(
            "INGESTER: Cycle done. {} blocks committed. Waiting {}s...",
            blocks_processed_this_cycle, POLL_INTERVAL_SECONDS
        );
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    info!("MAIN: Connecting to Ethereum node...");
    let rpc_url = env::var("ETH_RPC_URL")?;
    let provider = Arc::new(Provider::<Http>::try_from(rpc_url.as_str())?);
    info!("MAIN: Connected to Ethereum provider.");

    info!("MAIN: Connecting to database...");
    let database_url = env::var("DATABASE_URL")?;
    let pool: PgPool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    info!("MAIN: Connected to database.");

    let provider_for_ingester = provider.clone();
    let pool_for_ingester = pool.clone();

    tokio::spawn(async move {
        if let Err(e) = run_continuous_ingester(provider_for_ingester, pool_for_ingester).await {
            error!("CRITICAL: Ingester task exited with error: {}", e);
        } else {
            error!("Ingester task completed (should loop forever).");
        }
    });
    info!("MAIN: Ingester task spawned.");

    info!("MAIN: Starting API server...");
    if let Err(e) = api::run_api_server(pool).await {
        error!("CRITICAL: API server failed: {}", e);
        return Err(e);
    }

    Ok(())
}
