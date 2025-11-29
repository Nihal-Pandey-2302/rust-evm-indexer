// src/main.rs
mod api;
mod api_models;
mod db;
mod models;
mod docs;
use dotenvy::dotenv;
use ethers::{
    providers::{Http, Middleware, Provider}, // Middleware trait is needed for get_block_number, etc.
    types::U64,
};
use eyre::Result; // Using eyre::Result for main and ingester function
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;
use std::time::Duration;

use models::{MyBlock, MyLog, MyTransaction}; // Assuming these are still used by MyBlock/etc. mapping

// --- Constants for Ingester ---
const POLL_INTERVAL_SECONDS: u64 = 10; // Check for new blocks every 10 seconds
const BLOCKS_PER_BATCH: u64 = 5; // Process up to 5 blocks per cycle
const DEFAULT_START_BLOCK: u64 = 23900790; // Start from a recent block for testing
const MAX_RECEIPT_RETRIES: u32 = 3;
const BASE_RECEIPT_BACKOFF_SECONDS: u64 = 1;
const MAX_BLOCK_FETCH_RETRIES: u32 = 3;
const BASE_BLOCK_FETCH_BACKOFF_SECONDS: u64 = 2;

// --- New function for the continuous ingestion logic ---
async fn run_continuous_ingester(provider: Provider<Http>, pool: PgPool) -> Result<()> { // Using eyre::Result
    println!("\n--- Continuous Ingester Task Started ---");
    println!(
        "Polling for new blocks every {} seconds. Processing up to {} blocks per batch.",
        POLL_INTERVAL_SECONDS, BLOCKS_PER_BATCH
    );

    loop { // Outer loop for continuous polling
        let last_synced_block_opt = match db::get_last_synced_block(&pool).await {
            Ok(val) => val,
            Err(e) => {
                eprintln!("INGESTER DB: CRITICAL - Failed to get last synced block: {}. Retrying after {}s.", e, POLL_INTERVAL_SECONDS);
                tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
                continue;
            }
        };
        
        let start_block_to_fetch = match last_synced_block_opt {
            Some(last_block) => {
                last_block + 1
            }
            None => {
                println!(
                    "INGESTER: No last synced block found in DB. Starting from project default: {}",
                    DEFAULT_START_BLOCK
                );
                DEFAULT_START_BLOCK
            }
        };

        let current_chain_head = match provider.get_block_number().await {
            Ok(head) => head.as_u64(),
            Err(e) => {
                eprintln!("INGESTER ETH: CRITICAL - Failed to get current block number: {}. Retrying after {}s.", e, POLL_INTERVAL_SECONDS);
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
        
        if start_block_to_fetch > end_block_to_fetch {
             println!("INGESTER: No new blocks in the target range to form a full batch (Start: {}, Head: {}). Waiting...", start_block_to_fetch, current_chain_head);
             tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
             continue;
        }

        println!(
            "INGESTER Cycle: Targeting blocks from {} to {}. (Current Chain Head: {})",
            start_block_to_fetch, end_block_to_fetch, current_chain_head
        );

        let mut blocks_processed_this_cycle = 0;
        let mut latest_block_successfully_synced_this_cycle =
            last_synced_block_opt.unwrap_or(start_block_to_fetch.saturating_sub(1));

        for block_num_u64 in start_block_to_fetch..=end_block_to_fetch {
            let block_num_for_rpc = U64::from(block_num_u64);
            
            let block_processing_result = async { 
                let mut db_tx = pool.begin().await.map_err(|e| eyre::eyre!("DB: Failed to begin transaction for block {}: {}", block_num_u64, e))?;
                
                let mut ethers_block_option_from_rpc: Option<ethers::types::Block<ethers::types::Transaction>> = None;
                for attempt in 1..=MAX_BLOCK_FETCH_RETRIES {
                    match provider.get_block_with_txs(block_num_for_rpc).await {
                        Ok(Some(b)) => {
                            ethers_block_option_from_rpc = Some(b);
                            break; 
                        }
                        Ok(None) => {
                            eprintln!("INGESTER ETH: Block #{} not found (Ok(None)) by provider on attempt {}. This block will be skipped.", block_num_u64, attempt);
                            ethers_block_option_from_rpc = None;
                            break; 
                        }
                        Err(e) => {
                            eprintln!(
                                "INGESTER ETH: Attempt {}/{} failed to fetch block data for #{}: {:?}.",
                                attempt, MAX_BLOCK_FETCH_RETRIES, block_num_u64, e
                            );
                            if attempt == MAX_BLOCK_FETCH_RETRIES {
                                return Err(eyre::eyre!( 
                                    "Failed to fetch block data for #{} after {} attempts: {:?}",
                                    block_num_u64, MAX_BLOCK_FETCH_RETRIES, e
                                ));
                            }
                            let backoff_duration = Duration::from_secs(BASE_BLOCK_FETCH_BACKOFF_SECONDS * 2_u64.pow(attempt -1));
                            println!("INGESTER ETH: Retrying fetch for block #{} in {} seconds...", block_num_u64, backoff_duration.as_secs());
                            tokio::time::sleep(backoff_duration).await;
                        }
                    }
                }

                let ethers_block = match ethers_block_option_from_rpc {
                    Some(b) => b,
                    None => {
                        db_tx.commit().await.map_err(|e| eyre::eyre!("DB: Commit after skipping block {} (not found by provider) failed: {}", block_num_u64, e))?;
                        return Ok(false); 
                    }
                };

                let my_block = MyBlock { 
                    block_number: ethers_block.number.unwrap_or_default(),
                    block_hash: ethers_block.hash.unwrap_or_default(),
                    parent_hash: ethers_block.parent_hash,
                    timestamp: ethers_block.timestamp,
                    gas_used: ethers_block.gas_used,
                    gas_limit: ethers_block.gas_limit,
                    base_fee_per_gas: ethers_block.base_fee_per_gas,
                };
                db::insert_block_data(&mut db_tx, &my_block).await.map_err(|e| eyre::eyre!("DB: Insert block {} failed: {}", my_block.block_number, e))?;

                let transactions = ethers_block.transactions;
                let total_txs = transactions.len();
                for (idx, ethers_tx) in transactions.into_iter().enumerate() {
                    if idx % 20 == 0 || idx == total_txs - 1 {
                        println!("   -> Processing tx {}/{}...", idx + 1, total_txs);
                    }
                    let mut receipt_option_for_tx: Option<ethers::types::TransactionReceipt> = None;
                    for attempt in 1..=MAX_RECEIPT_RETRIES {
                        match provider.get_transaction_receipt(ethers_tx.hash).await {
                            Ok(r_opt) => {
                                receipt_option_for_tx = r_opt;
                                if receipt_option_for_tx.is_none() {
                                     println!("INGESTER ETH: No receipt found for tx {:?} (attempt {}/{}) in block {}, proceeding without receipt data.", ethers_tx.hash, attempt, MAX_RECEIPT_RETRIES, block_num_u64);
                                }
                                break; 
                            }
                            Err(e) => {
                                eprintln!("INGESTER ETH: Attempt {}/{} failed to fetch receipt for tx {:?} in block {}: {:?}.", attempt, MAX_RECEIPT_RETRIES, ethers_tx.hash, block_num_u64, e);
                                if attempt == MAX_RECEIPT_RETRIES {
                                    return Err(eyre::eyre!("Failed to fetch receipt for tx {:?} in block {} after {} attempts: {:?}", ethers_tx.hash, block_num_u64, MAX_RECEIPT_RETRIES, e));
                                }
                                let backoff_duration = Duration::from_secs(BASE_RECEIPT_BACKOFF_SECONDS * 2_u64.pow(attempt -1));
                                println!("INGESTER ETH: Retrying fetch for receipt of tx {:?} in {} seconds...", ethers_tx.hash, backoff_duration.as_secs());
                                tokio::time::sleep(backoff_duration).await;
                            }
                        }
                    }

                    let status = receipt_option_for_tx.as_ref().and_then(|r| r.status).map(|s| s.as_u64());
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
                    db::insert_transaction_data(&mut db_tx, &my_tx).await.map_err(|e| eyre::eyre!("DB: Insert tx {:?} failed: {}", my_tx.tx_hash, e))?;

                    if let Some(ref actual_receipt) = receipt_option_for_tx {
                        for ethers_log in &actual_receipt.logs {
                            let my_log = MyLog { 
                                log_index: ethers_log.log_index,
                                transaction_hash: ethers_log.transaction_hash.unwrap_or_default(),
                                transaction_index: ethers_log.transaction_index.map(|idx| idx.as_u64()),
                                block_number: ethers_log.block_number.map_or(0, |bn| bn.as_u64()),
                                block_hash: ethers_log.block_hash.unwrap_or_default(),
                                address: ethers_log.address,
                                data: ethers_log.data.to_string(),
                                topics: ethers_log.topics.iter().map(|h| format!("{:#x}", h)).collect(),
                             };
                            db::insert_log_data(&mut db_tx, &my_log).await.map_err(|e| eyre::eyre!("DB: Insert log for tx {:?} failed: {}", my_tx.tx_hash, e))?;
                        }
                    }
                } 

                db::set_last_synced_block(&mut db_tx, block_num_u64).await.map_err(|e| eyre::eyre!("DB: Set last_synced_block for {} failed: {}", block_num_u64, e))?;
                db_tx.commit().await.map_err(|e| eyre::eyre!("DB: Commit for block {} failed: {}", block_num_u64, e))?;
                
                // println!("INGESTER: Successfully committed and synced block #{}", block_num_u64); // More concise: use print!(".")
                Ok(true) 
            }.await;

            match block_processing_result {
                Ok(true) => { 
                    latest_block_successfully_synced_this_cycle = block_num_u64;
                    blocks_processed_this_cycle += 1;
                    print!("."); 
                    std::io::Write::flush(&mut std::io::stdout()).unwrap_or_default(); 
                }
                Ok(false) => { 
                    println!("\nINGESTER: Skipped processing for block #{} as it was not found by provider or deemed skippable.", block_num_u64);
                }
                Err(e) => { 
                    eprintln!("\nINGESTER: Failed to process block #{}: {}. Transaction rolled back. Will retry batch in next cycle.", block_num_u64, e);
                    break; 
                }
            }
            if block_processing_result.is_ok() {
                 tokio::time::sleep(Duration::from_millis(100)).await;
            }
        } 

        if blocks_processed_this_cycle > 0 {
            println!("\nINGESTER: Finished processing batch. {} blocks processed. Last successfully synced block in DB now: {}", blocks_processed_this_cycle, latest_block_successfully_synced_this_cycle);
        } else if start_block_to_fetch <= end_block_to_fetch { 
            println!("\nINGESTER: No blocks were successfully processed in this cycle (Target: {} to {}).", start_block_to_fetch, end_block_to_fetch);
        }

        println!(
            "INGESTER: Waiting {} seconds for next poll...\n",
            POLL_INTERVAL_SECONDS
        );
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
    } 
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // --- Setup Ethereum Provider ---
    println!("MAIN: Attempting to connect to Ethereum node...");
    let rpc_url = env::var("ETH_RPC_URL")?;
    let provider = Provider::<Http>::try_from(rpc_url.as_str())?;
    println!("MAIN: Successfully connected to Ethereum provider.");

    // --- Setup Database Pool ---
    println!("\nMAIN: Attempting to connect to database...");
    let database_url = env::var("DATABASE_URL")?;
    let pool: PgPool = PgPoolOptions::new()
        .max_connections(10) // Pool shared by ingester and API
        .connect(&database_url)
        .await?;
    println!("MAIN: Successfully connected to database.");

    // --- Clone resources for the ingester task ---
    let provider_for_ingester = provider.clone(); // Provider is Arc-based, clone is cheap
    let pool_for_ingester = pool.clone(); // PgPool is Arc-based, clone is cheap

    // --- Spawn the Ingester Task ---
    tokio::spawn(async move {
        // `move` captures the cloned provider and pool
        if let Err(e) = run_continuous_ingester(provider_for_ingester, pool_for_ingester).await {
            eprintln!("CRITICAL: Ingester task exited with error: {}", e);
            // In a real app, you might want to panic here or have a restart mechanism.
        } else {
            eprintln!("Ingester task completed (should typically loop forever).");
        }
    });
    println!("MAIN: Ingester task spawned and running in background.");

    // --- Start the API Server (runs in the main task) ---
    println!("MAIN: Starting API server...");
    if let Err(e) = api::run_api_server(pool).await {
        // Main task uses the original pool
        eprintln!("CRITICAL: API server failed: {}", e);
        return Err(e); // If API server fails, the whole application exits
    }

    Ok(())
}