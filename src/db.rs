// src/db.rs
use crate::models::{MyBlock, MyLog, MyTransaction};
use sqlx::{PgPool, Postgres, Transaction};

const INDEXER_NAME: &str = "evm_main_sync";

/// Gets the last canonical synced block number.
pub async fn get_last_synced_block(pool: &PgPool) -> Result<Option<u64>, sqlx::Error> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT last_processed_block FROM indexer_status WHERE indexer_name = $1")
            .bind(INDEXER_NAME)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0 as u64))
}

/// Returns the block_hash stored for the given block_number (most recently inserted).
/// Used for parent_hash validation to detect reorgs.
pub async fn get_canonical_block_hash_at_height(
    pool: &PgPool,
    block_number: u64,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT block_hash FROM blocks WHERE block_number = $1 ORDER BY block_number DESC LIMIT 1",
    )
    .bind(block_number as i64)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Deletes all blocks, transactions, and logs at or above `fork_height`.
/// Used during chain reorg rollback to restore a clean canonical state.
pub async fn rollback_from_height(pool: &PgPool, fork_height: u64) -> Result<(), sqlx::Error> {
    let height = fork_height as i64;
    // Order matters: delete dependent rows first
    sqlx::query("DELETE FROM logs WHERE block_number >= $1")
        .bind(height)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM transactions WHERE block_number >= $1")
        .bind(height)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM blocks WHERE block_number >= $1")
        .bind(height)
        .execute(pool)
        .await?;
    Ok(())
}

/// Updates indexer status: last synced block and the chain head at time of poll.
pub async fn set_last_synced_block(
    executor: &mut Transaction<'_, Postgres>,
    block_number: u64,
    chain_head: u64,
) -> Result<(), sqlx::Error> {
    let block_number_db = block_number as i64;
    let chain_head_db = chain_head as i64;

    sqlx::query(
        r#"
        INSERT INTO indexer_status (indexer_name, last_processed_block, chain_head_at_last_poll)
        VALUES ($1, $2, $3)
        ON CONFLICT (indexer_name) DO UPDATE SET
            last_processed_block = EXCLUDED.last_processed_block,
            chain_head_at_last_poll = EXCLUDED.chain_head_at_last_poll;
        "#,
    )
    .bind(INDEXER_NAME)
    .bind(block_number_db)
    .bind(chain_head_db)
    .execute(&mut **executor)
    .await?;

    Ok(())
}

/// Inserts block data. Conflict key is block_hash to support multiple blocks at same height.
pub async fn insert_block_data(
    executor: &mut Transaction<'_, Postgres>,
    block: &MyBlock,
) -> Result<(), sqlx::Error> {
    let block_hash_str = format!("{:#x}", block.block_hash);
    let parent_hash_str = format!("{:#x}", block.parent_hash);
    let timestamp_val = block.timestamp.as_u64() as i64;
    let gas_used_str = block.gas_used.to_string();
    let gas_limit_str = block.gas_limit.to_string();
    let base_fee_per_gas_str = block.base_fee_per_gas.map(|val| val.to_string());

    sqlx::query(
        r#"
        INSERT INTO blocks (
            block_hash, block_number, parent_hash, timestamp,
            gas_used, gas_limit, base_fee_per_gas
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7 )
        ON CONFLICT (block_hash) DO NOTHING;
        "#,
    )
    .bind(block_hash_str)
    .bind(block.block_number.as_u64() as i64)
    .bind(parent_hash_str)
    .bind(timestamp_val)
    .bind(gas_used_str)
    .bind(gas_limit_str)
    .bind(base_fee_per_gas_str)
    .execute(&mut **executor)
    .await?;
    Ok(())
}

pub async fn insert_transaction_data(
    executor: &mut Transaction<'_, Postgres>,
    tx: &MyTransaction,
) -> Result<(), sqlx::Error> {
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

    sqlx::query(
        r#"
        INSERT INTO transactions (
            tx_hash, block_number, block_hash, transaction_index,
            from_address, to_address, value, gas_price, max_fee_per_gas,
            max_priority_fee_per_gas, gas_provided, input_data, status
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13 )
        ON CONFLICT (tx_hash) DO NOTHING;
        "#,
    )
    .bind(tx_hash_str)
    .bind(block_number_val)
    .bind(block_hash_str)
    .bind(transaction_index_val)
    .bind(from_address_str)
    .bind(to_address_str)
    .bind(value_str)
    .bind(gas_price_str)
    .bind(max_fee_per_gas_str)
    .bind(max_priority_fee_per_gas_str)
    .bind(gas_provided_str)
    .bind(tx.input_data.as_bytes())
    .bind(status_val)
    .execute(&mut **executor)
    .await?;
    Ok(())
}

pub async fn insert_log_data(
    executor: &mut Transaction<'_, Postgres>,
    log: &MyLog,
) -> Result<(), sqlx::Error> {
    let tx_hash_str = format!("{:#x}", log.transaction_hash);
    let block_hash_str = format!("{:#x}", log.block_hash);
    let contract_address_str = format!("{:#x}", log.address);
    let topic0 = log.topics.first().map(|s| s.as_str());
    let topic1 = log.topics.get(1).map(|s| s.as_str());
    let topic2 = log.topics.get(2).map(|s| s.as_str());
    let topic3 = log.topics.get(3).map(|s| s.as_str());
    let log_index_val = log.log_index.map(|li| li.as_u64() as i64);
    let transaction_index_val = log.transaction_index.map(|ti| ti as i64);
    let block_number_val = log.block_number as i64;

    sqlx::query(
        r#"
        INSERT INTO logs (
            log_index_in_tx, transaction_hash, transaction_index_in_block,
            block_number, block_hash, contract_address, data,
            topic0, topic1, topic2, topic3, all_topics
        ) VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12 )
        "#,
    )
    .bind(log_index_val)
    .bind(tx_hash_str)
    .bind(transaction_index_val)
    .bind(block_number_val)
    .bind(block_hash_str)
    .bind(contract_address_str)
    .bind(log.data.as_bytes())
    .bind(topic0)
    .bind(topic1)
    .bind(topic2)
    .bind(topic3)
    .bind(&log.topics)
    .execute(&mut **executor)
    .await?;
    Ok(())
}

/// Returns the chain head and last synced block for lag computation.
pub async fn get_indexer_status(pool: &PgPool) -> Result<Option<(i64, i64)>, sqlx::Error> {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT last_processed_block, chain_head_at_last_poll FROM indexer_status WHERE indexer_name = $1"
    )
    .bind(INDEXER_NAME)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
