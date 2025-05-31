// src/db.rs
use crate::models::{MyBlock, MyLog, MyTransaction};
use sqlx::{PgPool, Postgres, Transaction}; // Added Transaction
                                           // Removed eyre::Result as functions will now primarily return sqlx::Error or standard Result for simplicity within DB operations
                                           // The caller (e.g., main.rs) can wrap these sqlx::Error into eyre::Report if needed.

const INDEXER_NAME: &str = "evm_main_sync";

// This function reads state and can still use the pool directly.
pub async fn get_last_synced_block(pool: &PgPool) -> Result<Option<u64>, sqlx::Error> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT last_processed_block FROM indexer_status WHERE indexer_name = $1")
            .bind(INDEXER_NAME)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|r| r.0 as u64))
}

// This function will now be part of a transaction, so it takes an Executor.
// For explicitness and common use within a transaction, we use &mut Transaction.
pub async fn set_last_synced_block(
    executor: &mut Transaction<'_, Postgres>, // Changed from &PgPool
    block_number: u64,
) -> Result<(), sqlx::Error> {
    let block_number_db = block_number as i64;

    sqlx::query(
        r#"
        INSERT INTO indexer_status (indexer_name, last_processed_block)
        VALUES ($1, $2)
        ON CONFLICT (indexer_name) DO UPDATE SET
            last_processed_block = EXCLUDED.last_processed_block;
        "#,
    )
    .bind(INDEXER_NAME)
    .bind(block_number_db)
    .execute(&mut **executor) // Use the transaction executor
    .await?;

    Ok(())
}

// Inserts block data into the 'blocks' table using a transaction.
pub async fn insert_block_data(
    executor: &mut Transaction<'_, Postgres>, // Changed from &PgPool
    block: &MyBlock,
) -> Result<(), sqlx::Error> {
    // Changed return type
    let block_hash_str = format!("{:#x}", block.block_hash);
    let parent_hash_str = format!("{:#x}", block.parent_hash);
    let timestamp_val = block.timestamp.as_u64() as i64;
    let gas_used_str = block.gas_used.to_string();
    let gas_limit_str = block.gas_limit.to_string();
    let base_fee_per_gas_str = block.base_fee_per_gas.map(|val| val.to_string());

    sqlx::query!(
        r#"
        INSERT INTO blocks (
            block_number, block_hash, parent_hash, timestamp,
            gas_used, gas_limit, base_fee_per_gas
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7 )
        ON CONFLICT (block_number) DO NOTHING;
        "#,
        block.block_number.as_u64() as i64,
        block_hash_str,
        parent_hash_str,
        timestamp_val,
        gas_used_str,
        gas_limit_str,
        base_fee_per_gas_str
    )
    .execute(&mut **executor) // Use the transaction executor
    .await?;
    // Removed println! from here; caller can log after successful transaction commit.
    Ok(())
}

// Inserts transaction data into the 'transactions' table using a transaction.
pub async fn insert_transaction_data(
    executor: &mut Transaction<'_, Postgres>, // Changed from &PgPool
    tx: &MyTransaction,
) -> Result<(), sqlx::Error> {
    // Changed return type
    let tx_hash_str = format!("{:#x}", tx.tx_hash);
    let block_hash_str = format!("{:#x}", tx.block_hash);
    let from_address_str = format!("{:#x}", tx.from_address);
    let to_address_str = tx.to_address.map(|addr| format!("{:#x}", addr));
    let value_str = tx.value.to_string();
    let gas_price_str = tx.gas_price.map(|gp| gp.to_string());
    let max_fee_per_gas_str = tx.max_fee_per_gas.map(|val| val.to_string());
    let max_priority_fee_per_gas_str = tx.max_priority_fee_per_gas.map(|val| val.to_string());
    let gas_provided_str = tx.gas.to_string();
    let block_number_val = tx.block_number.as_u64() as i64;
    let transaction_index_val = tx.transaction_index.map(|idx| idx.as_u64() as i64);
    let status_val = tx.status.map(|s| s as i16);

    sqlx::query!(
        r#"
        INSERT INTO transactions (
            tx_hash, block_number, block_hash, transaction_index,
            from_address, to_address, value, gas_price, max_fee_per_gas,
            max_priority_fee_per_gas, gas_provided, input_data, status
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13 )
        ON CONFLICT (tx_hash) DO NOTHING;
        "#,
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
        tx.input_data,
        status_val
    )
    .execute(&mut **executor) // Use the transaction executor
    .await?;
    Ok(())
}

// Inserts log data into the 'logs' table using a transaction.
pub async fn insert_log_data(
    executor: &mut Transaction<'_, Postgres>, // Changed from &PgPool
    log: &MyLog,
) -> Result<(), sqlx::Error> {
    // Changed return type
    let tx_hash_str = format!("{:#x}", log.transaction_hash);
    let block_hash_str = format!("{:#x}", log.block_hash);
    let contract_address_str = format!("{:#x}", log.address);
    let topic0 = log.topics.get(0).map(|s| s.as_str());
    let topic1 = log.topics.get(1).map(|s| s.as_str());
    let topic2 = log.topics.get(2).map(|s| s.as_str());
    let topic3 = log.topics.get(3).map(|s| s.as_str());
    let log_index_val = log.log_index.map(|li| li.as_u64() as i64);
    let transaction_index_val = log.transaction_index.map(|ti| ti as i64);
    let block_number_val = log.block_number as i64;

    sqlx::query!(
        r#"
        INSERT INTO logs (
            log_index_in_tx, transaction_hash, transaction_index_in_block,
            block_number, block_hash, contract_address, data,
            topic0, topic1, topic2, topic3, all_topics
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12 )
        ON CONFLICT (id) DO NOTHING;
        "#,
        log_index_val,
        tx_hash_str,
        transaction_index_val,
        block_number_val,
        block_hash_str,
        contract_address_str,
        log.data,
        topic0,
        topic1,
        topic2,
        topic3,
        &log.topics
    )
    .execute(&mut **executor) // Use the transaction executor
    .await?;
    Ok(())
}
