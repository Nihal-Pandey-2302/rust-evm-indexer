use axum::{
    extract::State,
    extract::Path,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};

use std::net::SocketAddr;
use sqlx::{Execute, PgPool, QueryBuilder};
use eyre::Result; // Or eyre::Report


// Import models needed by handlers
use crate::models::{MyBlock, MyTransaction, MyLog};
use crate::api_models::GetLogsFilter; // GetLogsFilter is used in get_logs_handler request


pub async fn root_handler() -> Html<&'static str> {
    Html("<h1>Hello, EVM Indexer API!</h1><p>Welcome to your Rust-powered API.</p>")
}

// src/main.rs
// ... (imports including MyLog, GetLogsFilter, PgPool, State, Json, StatusCode) ...

async fn get_logs_handler(
    State(pool): State<PgPool>,
    Json(filters): Json<GetLogsFilter>,
) -> Result<Json<Vec<MyLog>>, (StatusCode, String)> {
    println!("Received /logs request with filters: {:?}", filters);

    // Start building the query
    let mut query_builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, log_index_in_tx, transaction_hash, transaction_index_in_block, \
         block_number, block_hash, contract_address, data, \
         topic0, topic1, topic2, topic3, all_topics \
         FROM logs"
    );

    // Add WHERE clauses conditionally
    // We need a way to know if we've already added a WHERE clause to use AND correctly.
    // A common way is to add ` WHERE TRUE` or `WHERE 1=1` to the base query,
    // then all other conditions can start with `AND`.
    query_builder.push(" WHERE 1=1"); // Start with a clause that's always true

    if let Some(addr_filter) = &filters.address {
        query_builder.push(" AND LOWER(contract_address) = LOWER(");
        query_builder.push_bind(addr_filter.clone()); // Bind the address string
        query_builder.push(")");
    }

    if let Some(topic0_filter) = &filters.topic0 {
        query_builder.push(" AND LOWER(topic0) = LOWER(");
        query_builder.push_bind(topic0_filter.clone()); // Bind the topic0 string
        query_builder.push(")");
    }

    if let Some(fb) = filters.from_block {
        query_builder.push(" AND block_number >= ");
        query_builder.push_bind(fb as i64); // Bind from_block as i64
    }

    if let Some(tb) = filters.to_block {
        query_builder.push(" AND block_number <= ");
        query_builder.push_bind(tb as i64); // Bind to_block as i64
    }

    // Add ordering and limit
    query_builder.push(" ORDER BY block_number ASC, transaction_index_in_block ASC, log_index_in_tx ASC LIMIT 100");

    // Build the query
    let query = query_builder.build(); // This produces a Query<'_, Postgres, PgArguments>

    println!("Executing SQL: {}", query.sql()); // Print the generated SQL
    // Note: query.sql() will show $1, $2 etc. for bound parameters.

    let rows = match query.fetch_all(&pool).await {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("Failed to execute query: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database query error: {}", e),
            ));
        }
    };

    // Manual mapping from PgRow to MyLog (same logic as before)
    let mut logs_result: Vec<MyLog> = Vec::new();
    use std::str::FromStr; // For H256::from_str, Address::from_str
    use ethers::core::types::{H256, Address, U256}; // Ensure these are in scope
    use sqlx::Row; // To use row.try_get

    for row in rows {
        let parse_h256 = |s: Option<String>| -> H256 {
            s.and_then(|str_val| H256::from_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_address = |s: Option<String>| -> Address {
            s.and_then(|str_val| Address::from_str(&str_val).ok()).unwrap_or_default()
        };

        let topics_from_db: Vec<String> = row.try_get("all_topics").unwrap_or_default();

        let log_entry = MyLog {
            log_index: row.try_get::<Option<i64>, _>("log_index_in_tx").ok().flatten().map(U256::from),
            transaction_hash: parse_h256(row.try_get("transaction_hash").ok()),
            transaction_index: row.try_get::<Option<i64>, _>("transaction_index_in_block").ok().flatten().map(|v| v as u64),
            block_number: row.try_get::<i64, _>("block_number").map(|v| v as u64).unwrap_or(0),
            block_hash: parse_h256(row.try_get("block_hash").ok()),
            address: parse_address(row.try_get("contract_address").ok()),
            data: row.try_get("data").unwrap_or_else(|_| String::from("0x")),
            topics: topics_from_db,
        };
        logs_result.push(log_entry);
    }

    Ok(Json(logs_result))
}

async fn get_block_by_number_handler(
    State(pool): State<PgPool>,
    Path(block_number_param): Path<u64>, // Extract block_number from path
) -> Result<Json<MyBlock>, (StatusCode, String)> {
    println!("Received /block/{} request", block_number_param);

    // We need to convert block_number_param (u64) to i64 for the SQL query,
    // as our `blocks` table `block_number` is BIGINT.
    let block_number_db = block_number_param as i64;

    // Query the database for the block
    // Similar to get_logs_handler, direct query_as! would fail if MyBlock fields (H256, U256)
    // don't have FromRow implementations for SQL string/numeric types.
    // We'll fetch the row and manually map.

    let row_option = match sqlx::query(
        "SELECT block_number, block_hash, parent_hash, timestamp, gas_used, gas_limit, base_fee_per_gas \
         FROM blocks WHERE block_number = $1"
        )
        .bind(block_number_db)
        .fetch_optional(&pool) // Use fetch_optional to handle cases where block might not exist
        .await
    {
        Ok(row_opt) => row_opt,
        Err(e) => {
            eprintln!("DB error fetching block {}: {}", block_number_param, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)));
        }
    };

    if let Some(row) = row_option {
        // Manual mapping from PgRow to MyBlock
        use std::str::FromStr;
        use ethers::core::types::{H256, U256}; // Make sure these are in scope if not already
        use sqlx::Row;

        let parse_h256_from_str = |s: Option<String>| -> H256 {
            s.and_then(|str_val| H256::from_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_u256_from_text = |s: Option<String>| -> U256 { // U256 from TEXT
            s.and_then(|str_val| U256::from_dec_str(&str_val).ok()).unwrap_or_default()
        };
         let parse_option_u256_from_text = |s: Option<String>| -> Option<U256> {
            s.and_then(|str_val| U256::from_dec_str(&str_val).ok())
        };


        let my_block = MyBlock {
            block_number: row.try_get::<i64, _>("block_number").map(|n| n as u64).unwrap_or_default().into(), // DB BIGINT -> u64
            block_hash: parse_h256_from_str(row.try_get("block_hash").ok()),
            parent_hash: parse_h256_from_str(row.try_get("parent_hash").ok()),
            timestamp: U256::from(row.try_get::<i64, _>("timestamp").unwrap_or_default()), // DB BIGINT (Unix secs) -> U256
            gas_used: parse_u256_from_text(row.try_get("gas_used").ok()),
            gas_limit: parse_u256_from_text(row.try_get("gas_limit").ok()),
            base_fee_per_gas: parse_option_u256_from_text(row.try_get("base_fee_per_gas").ok()),
        };
        Ok(Json(my_block))
    } else {
        Err((StatusCode::NOT_FOUND, format!("Block #{} not found", block_number_param)))
    }
}

async fn get_transaction_by_hash_handler(
    State(pool): State<PgPool>,
    Path(tx_hash_param): Path<String>, // Extract tx_hash from path as String
) -> Result<Json<MyTransaction>, (StatusCode, String)> {
    println!("Received /transaction/{} request", tx_hash_param);

    // Ensure the input tx_hash_param starts with "0x" for consistency if your DB stores it that way.
    // Or, ensure your DB search is case-insensitive and handles with/without "0x".
    // For now, we assume the client sends a hex string that matches what's in the DB (e.g. "0x...").
    // Our DB stores tx_hash as VARCHAR(66).

    let row_option = match sqlx::query(
        "SELECT tx_hash, block_number, block_hash, transaction_index, \
         from_address, to_address, value, gas_price, max_fee_per_gas, \
         max_priority_fee_per_gas, gas_provided, input_data, status \
         FROM transactions WHERE tx_hash = $1"
        )
        .bind(&tx_hash_param) // Bind the transaction hash string
        .fetch_optional(&pool)
        .await
    {
        Ok(row_opt) => row_opt,
        Err(e) => {
            eprintln!("DB error fetching transaction {}: {}", tx_hash_param, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)));
        }
    };

    if let Some(row) = row_option {
        // Manual mapping from PgRow to MyTransaction
        use std::str::FromStr;
        use ethers::core::types::{H256, Address, U256, U64}; // Make sure U64 is imported if used for numbers
        use sqlx::Row;

        let parse_h256_from_str = |s: Option<String>| -> H256 {
            s.and_then(|str_val| H256::from_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_address_from_str = |s: Option<String>| -> Address {
            s.and_then(|str_val| Address::from_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_option_address_from_str = |s: Option<String>| -> Option<Address> {
            s.and_then(|str_val| Address::from_str(&str_val).ok())
        };
        let parse_u256_from_text = |s: Option<String>| -> U256 {
            s.and_then(|str_val| U256::from_dec_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_option_u256_from_text = |s: Option<String>| -> Option<U256> {
            s.and_then(|str_val| U256::from_dec_str(&str_val).ok())
        };

        // Assuming MyTransaction.block_number is U64 and transaction_index is Option<U64>
        // based on how they are assigned from ethers_tx in your ingestion logic.
        let my_tx = MyTransaction {
            tx_hash: parse_h256_from_str(row.try_get("tx_hash").ok()),
            block_number: U64::from(row.try_get::<i64, _>("block_number").unwrap_or_default()),
            block_hash: parse_h256_from_str(row.try_get("block_hash").ok()),
            transaction_index: row.try_get::<Option<i64>, _>("transaction_index").ok().flatten().map(U64::from),
            from_address: parse_address_from_str(row.try_get("from_address").ok()),
            to_address: parse_option_address_from_str(row.try_get("to_address").ok()),
            value: parse_u256_from_text(row.try_get("value").ok()),
            gas_price: parse_option_u256_from_text(row.try_get("gas_price").ok()),
            max_fee_per_gas: parse_option_u256_from_text(row.try_get("max_fee_per_gas").ok()),
            max_priority_fee_per_gas: parse_option_u256_from_text(row.try_get("max_priority_fee_per_gas").ok()),
            gas: parse_u256_from_text(row.try_get("gas_provided").ok()),
            input_data: row.try_get("input_data").unwrap_or_default(),
            status: row.try_get::<Option<i16>, _>("status").ok().flatten().map(|s| s as u64),
        };
        Ok(Json(my_tx))
    } else {
        Err((StatusCode::NOT_FOUND, format!("Transaction {} not found", tx_hash_param)))
    }
}


pub(crate) async fn run_api_server(pool: PgPool) -> eyre::Result<()> {
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/logs", post(get_logs_handler))
        .route("/block/{block_number}", get(get_block_by_number_handler))
        .route("/transaction/{tx_hash}", get(get_transaction_by_hash_handler)) // **** NEW ROUTE ****
        .with_state(pool.clone());

    // ... rest of the function ...
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("API server listening on http://{}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
        .await
        .map_err(|e| eyre::eyre!("API server error: {}", e))?;
    
    Ok(())
}