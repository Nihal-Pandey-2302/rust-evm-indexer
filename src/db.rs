use crate::models::{MyBlock, MyLog, MyTransaction}; // Use `crate::models`
use eyre::Result;
use sqlx::PgPool;

// Inserts block data into the 'blocks' table.
pub async fn insert_block_data(pool: &PgPool, block: &MyBlock) -> Result<(), eyre::Report> {
    // Prepare string versions of hash types for SQL TEXT/VARCHAR columns.
    let block_hash_str = format!("{:#x}", block.block_hash);
    let parent_hash_str = format!("{:#x}", block.parent_hash);

    // Prepare numeric and potentially large number types for SQL.
    // Timestamp is stored as Unix epoch seconds (BIGINT).
    let timestamp_val = block.timestamp.as_u64() as i64;
    let gas_used_str = block.gas_used.to_string(); // U256 -> TEXT
    let gas_limit_str = block.gas_limit.to_string(); // U256 -> TEXT
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
pub async fn insert_transaction_data(
    pool: &PgPool,
    tx: &MyTransaction,
) -> Result<(), eyre::Report> {
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
pub async fn insert_log_data(pool: &PgPool, log: &MyLog) -> Result<(), eyre::Report> {
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
