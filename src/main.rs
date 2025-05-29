mod models;

use dotenvy::dotenv;
use ethers::{
    providers::{Http, Middleware, Provider},
    types::U64, // For Ethereum U64 type, typically used for block numbers
};
use eyre::Result;
use std::env;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use models::{MyBlock, MyTransaction, MyLog};

// Inserts block data into the 'blocks' table.
async fn insert_block_data(pool: &PgPool, block: &models::MyBlock) -> Result<(), eyre::Report> {
    // Prepare string versions of hash types for SQL TEXT/VARCHAR columns.
    let block_hash_str = format!("{:#x}", block.block_hash);
    let parent_hash_str = format!("{:#x}", block.parent_hash);

    // Prepare numeric and potentially large number types for SQL.
    // Timestamp is stored as Unix epoch seconds (BIGINT).
    let timestamp_val = block.timestamp.as_u64() as i64;
    let gas_used_str = block.gas_used.to_string();       // U256 -> TEXT
    let gas_limit_str = block.gas_limit.to_string();      // U256 -> TEXT
    let base_fee_per_gas_str = block.base_fee_per_gas.map(|val| val.to_string()); // Option<U256> -> Option<String>

    sqlx::query!(
        r#"
        INSERT INTO blocks (
            block_number, block_hash, parent_hash, timestamp,
            gas_used, gas_limit, base_fee_per_gas
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7 )
        ON CONFLICT (block_number) DO NOTHING; 
        "#, // Idempotent insert: ignore if block_number already exists.
        block.block_number.as_u64() as i64, // MyBlock.block_number is U64, db column is BIGINT
        block_hash_str,
        parent_hash_str,
        timestamp_val,
        gas_used_str,
        gas_limit_str,
        base_fee_per_gas_str
    )
    .execute(pool)
    .await
    .map_err(|e| eyre::eyre!("DB: Failed to insert block {}: {}", block.block_number, e))?;

    println!("DB: Inserted block #{}", block.block_number);
    Ok(())
}

// Inserts transaction data into the 'transactions' table.
async fn insert_transaction_data(pool: &PgPool, tx: &models::MyTransaction) -> Result<(), eyre::Report> {
    let tx_hash_str = format!("{:#x}", tx.tx_hash);
    let block_hash_str = format!("{:#x}", tx.block_hash); // Denormalized for convenience
    let from_address_str = format!("{:#x}", tx.from_address);
    let to_address_str = tx.to_address.map(|addr| format!("{:#x}", addr));

    let value_str = tx.value.to_string();
    let gas_price_str = tx.gas_price.map(|gp| gp.to_string());
    let max_fee_per_gas_str = tx.max_fee_per_gas.map(|val| val.to_string());
    let max_priority_fee_per_gas_str = tx.max_priority_fee_per_gas.map(|val| val.to_string());
    let gas_provided_str = tx.gas.to_string();

    // Map struct types (U64, Option<U64>, Option<u64>) to SQL compatible types (i64, Option<i64>, Option<i16>).
    let block_number_val = tx.block_number.as_u64() as i64;
    let transaction_index_val = tx.transaction_index.map(|idx| idx.as_u64() as i64);
    let status_val = tx.status.map(|s| s as i16); // Stored as SMALLINT in DB

    sqlx::query!(
        r#"
        INSERT INTO transactions (
            tx_hash, block_number, block_hash, transaction_index,
            from_address, to_address, value, gas_price, max_fee_per_gas,
            max_priority_fee_per_gas, gas_provided, input_data, status
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13 )
        ON CONFLICT (tx_hash) DO NOTHING;
        "#, // Idempotent insert: ignore if tx_hash already exists.
        tx_hash_str,
        block_number_val,
        block_hash_str,
        transaction_index_val,
        from_address_str,
        to_address_str,
        value_str,
        gas_price_str,
        max_fee_per_gas_str,
        max_priority_fee_per_gas_str,
        gas_provided_str,
        tx.input_data, // input_data is already String in MyTransaction
        status_val
    )
    .execute(pool)
    .await
    .map_err(|e| eyre::eyre!("DB: Failed to insert transaction {}: {}", tx_hash_str, e))?;
    Ok(())
}

// Inserts log data into the 'logs' table.
async fn insert_log_data(pool: &PgPool, log: &models::MyLog) -> Result<(), eyre::Report> {
    let tx_hash_str = format!("{:#x}", log.transaction_hash);
    let block_hash_str = format!("{:#x}", log.block_hash);
    let contract_address_str = format!("{:#x}", log.address);

    // Extract individual topics for dedicated SQL columns; topics are already hex strings.
    let topic0 = log.topics.get(0).map(|s| s.as_str());
    let topic1 = log.topics.get(1).map(|s| s.as_str());
    let topic2 = log.topics.get(2).map(|s| s.as_str());
    let topic3 = log.topics.get(3).map(|s| s.as_str());

    // Map struct types to SQL compatible types.
    let log_index_val = log.log_index.map(|li| li.as_u64() as i64); // Option<U256> -> Option<i64>
    let transaction_index_val = log.transaction_index.map(|ti| ti as i64); // Option<u64> -> Option<i64>
    let block_number_val = log.block_number as i64; // u64 -> i64

    sqlx::query!(
        r#"
        INSERT INTO logs (
            log_index_in_tx, transaction_hash, transaction_index_in_block,
            block_number, block_hash, contract_address, data,
            topic0, topic1, topic2, topic3, all_topics
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12 )
        ON CONFLICT (id) DO NOTHING; 
        "#, // Relies on 'id' BIGSERIAL for conflict. Consider unique constraint on (tx_hash, log_index_in_tx) for content-based idempotency.
        log_index_val,
        tx_hash_str,
        transaction_index_val,
        block_number_val,
        block_hash_str,
        contract_address_str,
        log.data, // log.data is already String
        topic0,
        topic1,
        topic2,
        topic3,
        &log.topics // sqlx handles Vec<String> to TEXT[] mapping for PostgreSQL
    )
    .execute(pool)
    .await
    .map_err(|e| eyre::eyre!("DB: Failed to insert log for tx {}: {:?}", tx_hash_str, e))?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok(); // Load .env file (if present)

    // Establish Ethereum node connection
    println!("Attempting to connect to Ethereum node...");
    let rpc_url = env::var("ETH_RPC_URL")?; // Error out if not found
    println!("Using RPC URL: {}", rpc_url);
    let provider = Provider::<Http>::try_from(rpc_url.as_str())?;
    println!("Successfully connected to Ethereum provider.");

    // Establish database connection pool
    println!("\nAttempting to connect to database...");
    let database_url = env::var("DATABASE_URL")?; // Error out if not found
    let pool: PgPool = PgPoolOptions::new()
        .max_connections(5) // Configure max connections for the pool
        .connect(&database_url)
        .await?;
    println!("Successfully connected to database.");

    // Simple DB test query
    let test_query_result: (i32,) = sqlx::query_as("SELECT 1 AS test_value")
        .fetch_one(&pool)
        .await?;
    println!("Database test query result (should be 1): {}", test_query_result.0);

    // Fetching and processing blockchain data
    let current_block_number = provider.get_block_number().await?;
    println!("\nCurrent Ethereum block number: {}", current_block_number);

    let num_blocks_to_fetch = 1; // Number of recent blocks to fetch and process
    let start_block_num_u64 = current_block_number.as_u64().saturating_sub(num_blocks_to_fetch -1);

    println!(
        "Fetching blocks from {} to {}",
        start_block_num_u64,
        current_block_number.as_u64()
    );

    // In-memory collections for data (primarily for printing summary)
    let mut all_my_blocks: Vec<MyBlock> = Vec::new();
    let mut all_my_transactions: Vec<MyTransaction> = Vec::new();
    let mut all_my_logs: Vec<MyLog> = Vec::new();

    for block_num_u64 in (start_block_num_u64..=current_block_number.as_u64()).rev() {
        let block_num_for_rpc = U64::from(block_num_u64);
        println!("\nFetching data for block: {}", block_num_for_rpc);

        match provider.get_block_with_txs(block_num_for_rpc).await {
            Ok(Some(ethers_block)) => {
                // Map to custom block struct
                let my_block = MyBlock {
                    // Assuming MyBlock.block_number is U64, matching ethers_block.number type
                    block_number: ethers_block.number.unwrap_or_default(),
                    block_hash: ethers_block.hash.unwrap_or_default(),
                    parent_hash: ethers_block.parent_hash,
                    timestamp: ethers_block.timestamp,
                    gas_used: ethers_block.gas_used,
                    gas_limit: ethers_block.gas_limit,
                    base_fee_per_gas: ethers_block.base_fee_per_gas,
                };
                all_my_blocks.push(my_block.clone());
                insert_block_data(&pool, &my_block).await?; 

                for ethers_tx in ethers_block.transactions {
                    // Fetching receipts is N+1, can be slow for many transactions.
                    let receipt_option = provider.get_transaction_receipt(ethers_tx.hash).await?;
                    let status = receipt_option.as_ref().and_then(|r| r.status).map(|s| s.as_u64());

                    // Map to custom transaction struct
                    let my_tx = MyTransaction {
                        // Assuming MyTransaction fields match ethers_tx types where appropriate (e.g., U64 for numbers)
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
                    all_my_transactions.push(my_tx.clone());
                    insert_transaction_data(&pool, &my_tx).await?;

                    if let Some(receipt) = receipt_option {
                        for ethers_log in receipt.logs {
                            // Map to custom log struct
                            let my_log = MyLog {
                                log_index: ethers_log.log_index,
                                transaction_hash: ethers_log.transaction_hash.unwrap_or_default(),
                                transaction_index: ethers_log.transaction_index.map(|idx| idx.as_u64()),
                                block_number: ethers_log.block_number.map_or(0, |bn| bn.as_u64()),
                                block_hash: ethers_log.block_hash.unwrap_or_default(),
                                address: ethers_log.address,
                                data: ethers_log.data.to_string(),
                                topics: ethers_log.topics.into_iter().map(|h| format!("{:#x}", h)).collect(),
                            };
                            insert_log_data(&pool, &my_log).await?;
                            all_my_logs.push(my_log);
                        }
                    }
                }
            }
            Ok(None) => {
                eprintln!("Block #{} not found (None returned).", block_num_for_rpc);
            }
            Err(e) => {
                eprintln!("Error fetching block #{}: {:?}", block_num_for_rpc, e);
            }
        }
        // Brief pause to be kind to the RPC provider.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Print summaries of processed data
    println!("\n--- Processed Blocks ---");
    for b in all_my_blocks.iter().take(2) {
        println!("{:#?}", b);
    }
    if all_my_blocks.len() > 2 {
        println!("... and {} more blocks.", all_my_blocks.len() - 2);
    }

    println!("\n--- Processed Transactions (first few) ---");
    for t in all_my_transactions.iter().take(5) {
        println!("{:#?}", t);
    }
    if all_my_transactions.len() > 5 {
        println!("... and {} more transactions.", all_my_transactions.len() - 5);
    }

    println!("\n--- Processed Logs (first few) ---");
    for l in all_my_logs.iter().take(5) {
        println!("{:#?}", l);
    }
    if all_my_logs.len() > 5 {
        println!("... and {} more logs.", all_my_logs.len() - 5);
    }

    Ok(())
}
