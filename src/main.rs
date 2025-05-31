// src/main.rs
mod api;
mod api_models;
mod db;
mod models;

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
const BLOCKS_PER_BATCH: u64 = 5;      // Process up to 5 blocks per cycle
const DEFAULT_START_BLOCK: u64 = 0; // Or your desired actual genesis/start block for a full sync

// --- New function for the continuous ingestion logic ---
async fn run_continuous_ingester(provider: Provider<Http>, pool: PgPool) -> Result<()> {
    println!("\n--- Continuous Ingester Task Started ---");
    println!("Polling for new blocks every {} seconds. Processing up to {} blocks per batch.", POLL_INTERVAL_SECONDS, BLOCKS_PER_BATCH);

    loop { // Outer loop for continuous polling
        // 1. Get the last synced block from the DB
        let last_synced_block_opt = match db::get_last_synced_block(&pool).await {
            Ok(val) => val,
            Err(e) => {
                eprintln!("INGESTER DB: CRITICAL - Failed to get last synced block: {}. Retrying after {}s.", e, POLL_INTERVAL_SECONDS);
                tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
                continue; // Skip to next iteration of the outer loop
            }
        };
        
        let start_block_to_fetch = match last_synced_block_opt {
            Some(last_block) => {
                // println!("INGESTER: Resuming from last synced block in DB: {}. Will fetch from {}.", last_block, last_block + 1);
                last_block + 1
            }
            None => {
                println!("INGESTER: No last synced block found in DB. Starting from project default: {}", DEFAULT_START_BLOCK);
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
            // println!("INGESTER: Up to date with chain head (Next to fetch: {}, Head: {}). Waiting for new blocks...", start_block_to_fetch, current_chain_head);
            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
            continue; 
        }
        
        let end_block_to_fetch = (start_block_to_fetch + BLOCKS_PER_BATCH - 1).min(current_chain_head);
        
        println!("INGESTER Cycle: Targeting blocks from {} to {}. (Current Chain Head: {})", start_block_to_fetch, end_block_to_fetch, current_chain_head);

        let mut blocks_processed_this_cycle = 0;
        let mut latest_block_successfully_synced_this_cycle = last_synced_block_opt.unwrap_or(start_block_to_fetch.saturating_sub(1));

        for block_num_u64 in start_block_to_fetch..=end_block_to_fetch {
            let block_num_for_rpc = U64::from(block_num_u64);
            
            let mut db_tx = match pool.begin().await {
                Ok(tx) => tx,
                Err(e) => {
                    eprintln!("INGESTER DB: Failed to begin transaction for block {}: {}. Retrying cycle.", block_num_u64, e);
                    break; 
                }
            };
            
            // println!("INGESTER: Processing block: {}", block_num_u64); // Verbose

            match provider.get_block_with_txs(block_num_for_rpc).await {
                Ok(Some(ethers_block)) => {
                    let my_block = MyBlock { /* ... your full mapping ... */ 
                        block_number: ethers_block.number.unwrap_or_default(),
                        block_hash: ethers_block.hash.unwrap_or_default(),
                        parent_hash: ethers_block.parent_hash,
                        timestamp: ethers_block.timestamp,
                        gas_used: ethers_block.gas_used,
                        gas_limit: ethers_block.gas_limit,
                        base_fee_per_gas: ethers_block.base_fee_per_gas,
                    };
                    if let Err(e) = db::insert_block_data(&mut db_tx, &my_block).await {
                        eprintln!("INGESTER DB: Error inserting block data for #{}: {:?}. Rolling back.", block_num_u64, e);
                        // Rollback is implicit on drop if not committed
                        break; 
                    }

                    for ethers_tx in ethers_block.transactions {
                        let receipt_option = match provider.get_transaction_receipt(ethers_tx.hash).await {
                            Ok(r_opt) => r_opt,
                            Err(e) => {
                                eprintln!("INGESTER ETH: Error fetching receipt for tx {:?} in block {}: {:?}. Rolling back block.", ethers_tx.hash, block_num_u64, e);
                                // Break from the transaction loop for this block, db_tx will rollback.
                                // Set a flag or return an error that the outer loop can catch to break.
                                // For simplicity, we will let the transaction rollback and retry the block.
                                return Err(eyre::eyre!("Receipt fetch failed for block {}", block_num_u64)); // Exit this task to be restarted or handled
                            }
                        };

                        let status = receipt_option.as_ref().and_then(|r| r.status).map(|s| s.as_u64());
                        let my_tx = MyTransaction { /* ... your full mapping ... */ 
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
                        if let Err(e) = db::insert_transaction_data(&mut db_tx, &my_tx).await {
                            eprintln!("INGESTER DB: Error inserting tx data for {:?} in block {}: {:?}. Rolling back.", my_tx.tx_hash, block_num_u64, e);
                            return Err(eyre::eyre!("DB insert tx failed for block {}", block_num_u64));
                        }

                        if let Some(receipt) = receipt_option {
                            for ethers_log in receipt.logs {
                                let my_log = MyLog { /* ... your full mapping ... */ 
                                    log_index: ethers_log.log_index,
                                    transaction_hash: ethers_log.transaction_hash.unwrap_or_default(),
                                    transaction_index: ethers_log.transaction_index.map(|idx| idx.as_u64()),
                                    block_number: ethers_log.block_number.map_or(0, |bn| bn.as_u64()),
                                    block_hash: ethers_log.block_hash.unwrap_or_default(),
                                    address: ethers_log.address,
                                    data: ethers_log.data.to_string(),
                                    topics: ethers_log.topics.into_iter().map(|h| format!("{:#x}", h)).collect(),
                                };
                                if let Err(e) = db::insert_log_data(&mut db_tx, &my_log).await {
                                    eprintln!("INGESTER DB: Error inserting log data for tx {:?} in block {}: {:?}. Rolling back.", my_tx.tx_hash, block_num_u64, e);
                                    return Err(eyre::eyre!("DB insert log failed for block {}", block_num_u64));
                                }
                            }
                        }
                    }

                    if let Err(e) = db::set_last_synced_block(&mut db_tx, block_num_u64).await {
                        eprintln!("INGESTER DB: CRITICAL - Failed to set last synced block to {} in transaction: {}. Rolling back.", block_num_u64, e);
                        return Err(eyre::eyre!("DB update last_synced_block failed for block {}", block_num_u64));
                    }
                    
                    if let Err(e) = db_tx.commit().await {
                        eprintln!("INGESTER DB: CRITICAL - Failed to commit transaction for block {}: {}. State might be inconsistent.", block_num_u64, e);
                        return Err(eyre::eyre!("DB commit failed for block {}", block_num_u64));
                    }

                    println!("INGESTER: Successfully committed and synced block #{}", block_num_u64);
                    latest_block_successfully_synced_this_cycle = block_num_u64;
                    blocks_processed_this_cycle += 1;
                }
                Ok(None) => { 
                    eprintln!("INGESTER ETH: Block #{} not found. Continuing to next block in batch.", block_num_for_rpc);
                }
                Err(e) => {
                    eprintln!("INGESTER ETH: Error fetching block data for #{}: {:?}. Will retry batch in next cycle.", block_num_for_rpc, e);
                    break; 
                }
            }
            // Only sleep if we successfully processed the block or it was None. Don't sleep if an error broke the loop early.
            if blocks_processed_this_cycle > 0 && block_num_u64 == latest_block_successfully_synced_this_cycle { // checks if current block was the one last synced
                 tokio::time::sleep(Duration::from_millis(100)).await;
            } else if provider.get_block_with_txs(block_num_for_rpc).await.is_ok() { // if no error but also not synced (e.g. Ok(None))
                 tokio::time::sleep(Duration::from_millis(100)).await;
            }

        } 

        if blocks_processed_this_cycle > 0 {
            println!("INGESTER: Finished processing batch. {} blocks processed. Last synced in DB: {}", blocks_processed_this_cycle, latest_block_successfully_synced_this_cycle);
        } else if start_block_to_fetch <= current_chain_head {
            println!("INGESTER: No blocks processed in this cycle (Target: {} to {}). Possibly due to errors or no new blocks in range.", start_block_to_fetch, end_block_to_fetch);
        }

        println!("INGESTER: Waiting {} seconds for next poll...\n", POLL_INTERVAL_SECONDS);
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
    }
    // This function, when running in a loop, effectively doesn't "complete" unless an unrecoverable error
    // propagates out of it, or the task is cancelled.
    // For this example, if a critical DB error occurs, it returns Err, stopping this task.
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
    let pool_for_ingester = pool.clone();         // PgPool is Arc-based, clone is cheap

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
    if let Err(e) = api::run_api_server(pool).await { // Main task uses the original pool
        eprintln!("CRITICAL: API server failed: {}", e);
        return Err(e); // If API server fails, the whole application exits
    }

    Ok(())
}