use axum::{
    extract::Path,
    extract::State,
    http::StatusCode,
    response::IntoResponse, // Important for custom error types
    response::{Html, Json},
    routing::{get, post},
    Router,
};

use eyre::Result; // Or eyre::Report
use serde_json::json;
use sqlx::{Execute, PgPool, QueryBuilder};
use std::net::SocketAddr;

// Import models needed by handlers
use crate::api_models::GetLogsFilter;
use crate::models::{MyBlock, MyLog, MyTransaction}; // GetLogsFilter is used in get_logs_handler request

#[derive(Debug)] // Allow printing the error for debugging
pub enum ApiError {
    NotFound(String),
    InternalServerError(String),
    DatabaseError(sqlx::Error), // Store the original sqlx::Error
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            ApiError::InternalServerError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
            ApiError::DatabaseError(db_err) => {
                // Log the detailed database error for server-side debugging
                eprintln!("Database error: {:?}", db_err);
                // Return a more generic message to the client
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "A database error occurred".to_string(),
                )
            }
        };

        let body = Json(json!({
            "status": "error", // Or "fail" depending on your preferred error structure
            "statusCode": status.as_u16(),
            "message": error_message,
        }));

        (status, body).into_response()
    }
}

// Helper: Convert sqlx::Error into ApiError
// This allows you to use `?` on sqlx results in handlers that return Result<_, ApiError>
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError::DatabaseError(err)
    }
}

impl From<eyre::Report> for ApiError {
    fn from(err: eyre::Report) -> Self {
        eprintln!("Internal server error (eyre): {:?}", err);
        ApiError::InternalServerError("An internal server error occurred".to_string())
    }
}

pub async fn root_handler() -> Html<&'static str> {
    Html("<h1>Hello, EVM Indexer API!</h1><p>Welcome to your Rust-powered API.</p>")
}

// src/main.rs
// ... (imports including MyLog, GetLogsFilter, PgPool, State, Json, StatusCode) ...

async fn get_logs_handler(
    State(pool): State<PgPool>,
    Json(filters): Json<GetLogsFilter>,
) -> Result<Json<Vec<MyLog>>, ApiError> {
    // <--- CHANGED return type
    println!("Received /logs request with filters: {:?}", filters);

    let mut query_builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, log_index_in_tx, transaction_hash, transaction_index_in_block, \
         block_number, block_hash, contract_address, data, \
         topic0, topic1, topic2, topic3, all_topics \
         FROM logs",
    );
    query_builder.push(" WHERE 1=1");

    if let Some(addr_filter) = &filters.address {
        query_builder.push(" AND LOWER(contract_address) = LOWER(");
        query_builder.push_bind(addr_filter.clone());
        query_builder.push(")");
    }
    if let Some(topic0_filter) = &filters.topic0 {
        query_builder.push(" AND LOWER(topic0) = LOWER(");
        query_builder.push_bind(topic0_filter.clone());
        query_builder.push(")");
    }
    if let Some(fb) = filters.from_block {
        query_builder.push(" AND block_number >= ");
        query_builder.push_bind(fb as i64);
    }
    if let Some(tb) = filters.to_block {
        query_builder.push(" AND block_number <= ");
        query_builder.push_bind(tb as i64);
    }
    query_builder.push(
        " ORDER BY block_number ASC, transaction_index_in_block ASC, log_index_in_tx ASC LIMIT 100",
    );

    let query = query_builder.build();
    println!("Executing SQL: {}", query.sql());

    let rows = query.fetch_all(&pool).await?; // <--- The ? uses From<sqlx::Error> for ApiError

    let mut logs_result: Vec<MyLog> = Vec::new();

    // Import necessary types and traits into the function's scope
    use ethers::core::types::{Address, H256, U256};
    use sqlx::Row as SqlxRow;
    use std::str::FromStr;

    // Helper closures (ensure ethers types and FromStr are in scope)
    let parse_h256_from_str = |s_opt: Option<String>| -> H256 {
        s_opt
            .and_then(|s| H256::from_str(&s).ok())
            .unwrap_or_default()
    };
    let parse_address_from_str = |s_opt: Option<String>| -> Address {
        s_opt
            .and_then(|s| Address::from_str(&s).ok())
            .unwrap_or_default()
    };
    // ... other necessary parsing helpers ...

    for row in rows {
        let topics_from_db: Vec<String> = SqlxRow::try_get(&row, "all_topics").unwrap_or_default(); // Or handle error

        let log_entry = MyLog {
            log_index: SqlxRow::try_get::<Option<i64>, _>(&row, "log_index_in_tx")
                .ok()
                .flatten()
                .map(U256::from),
            transaction_hash: parse_h256_from_str(SqlxRow::try_get(&row, "transaction_hash").ok()),
            transaction_index: SqlxRow::try_get::<Option<i64>, _>(
                &row,
                "transaction_index_in_block",
            )
            .ok()
            .flatten()
            .map(|v| v as u64),
            block_number: SqlxRow::try_get::<i64, _>(&row, "block_number")
                .map(|v| v as u64)
                .unwrap_or(0),
            block_hash: parse_h256_from_str(SqlxRow::try_get(&row, "block_hash").ok()),
            address: parse_address_from_str(SqlxRow::try_get(&row, "contract_address").ok()),
            data: SqlxRow::try_get(&row, "data").unwrap_or_else(|_| String::from("0x")), // Provide a default for data if missing/error
            topics: topics_from_db,
        };
        logs_result.push(log_entry);
    }

    Ok(Json(logs_result))
}

async fn get_block_by_number_handler(
    State(pool): State<PgPool>,
    Path(block_number_param): Path<u64>,
) -> Result<Json<MyBlock>, ApiError> {
    // <--- CHANGED return type
    println!("Received /block/{} request", block_number_param);

    let block_number_db = block_number_param as i64;

    let row_option = sqlx::query(
        "SELECT block_number, block_hash, parent_hash, timestamp, gas_used, gas_limit, base_fee_per_gas \
         FROM blocks WHERE block_number = $1"
        )
        .bind(block_number_db)
        .fetch_optional(&pool)
        .await?; // The ? will now use our From<sqlx::Error> for ApiError if there's a DB connection error

    if let Some(row) = row_option {
        // Manual mapping from PgRow to MyBlock
        use std::str::FromStr; // Required for H256::from_str
        use ethers::core::types::{H256, U256, U64}; // U64 for block_number, Address removed as unused
        use sqlx::Row as SqlxRow;

        let parse_h256_from_str = |s: Option<String>| -> H256 {
            s.and_then(|str_val| H256::from_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_u256_from_text = |s: Option<String>| -> U256 {
            s.and_then(|str_val| U256::from_dec_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_option_u256_from_text = |s: Option<String>| -> Option<U256> {
            s.and_then(|str_val| U256::from_dec_str(&str_val).ok())
        };

        let my_block = MyBlock {
            block_number: SqlxRow::try_get::<i64, _>(&row, "block_number").map(|n| U64::from(n as u64)).unwrap_or_default(),
            block_hash: parse_h256_from_str(SqlxRow::try_get(&row, "block_hash").ok()),
            parent_hash: parse_h256_from_str(SqlxRow::try_get(&row, "parent_hash").ok()),
            timestamp: U256::from(SqlxRow::try_get::<i64, _>(&row, "timestamp").unwrap_or_default()),
            gas_used: parse_u256_from_text(SqlxRow::try_get(&row, "gas_used").ok()),
            gas_limit: parse_u256_from_text(SqlxRow::try_get(&row, "gas_limit").ok()),
            base_fee_per_gas: parse_option_u256_from_text(SqlxRow::try_get(&row, "base_fee_per_gas").ok()),
        };
        Ok(Json(my_block))
    } else {
        // Return our custom ApiError variant
        Err(ApiError::NotFound(format!(
            "Block #{} not found",
            block_number_param
        )))
    }
}

async fn get_transaction_by_hash_handler(
    State(pool): State<PgPool>,
    Path(tx_hash_param): Path<String>,
) -> Result<Json<MyTransaction>, ApiError> {
    // <--- CHANGED return type
    println!("Received /transaction/{} request", tx_hash_param);

    // Prepare the tx_hash for query, ensuring it's consistently formatted if necessary
    // For now, assume tx_hash_param is a "0x..." hex string
    let mut formatted_tx_hash = tx_hash_param.clone();
    if !formatted_tx_hash.starts_with("0x") {
        formatted_tx_hash = format!("0x{}", formatted_tx_hash);
    }
    // You might also want to convert to lowercase for case-insensitive matching,
    // depending on how you store/query hashes. Let's assume DB handles it or it's stored consistently.

    let row_option = sqlx::query(
        "SELECT tx_hash, block_number, block_hash, transaction_index, \
         from_address, to_address, value, gas_price, max_fee_per_gas, \
         max_priority_fee_per_gas, gas_provided, input_data, status \
         FROM transactions WHERE tx_hash = $1",
    )
    .bind(formatted_tx_hash.to_lowercase()) // Example: bind lowercase if DB stores lowercase
    .fetch_optional(&pool)
    .await?; // The ? uses From<sqlx::Error> for ApiError

    if let Some(row) = row_option {
        use std::str::FromStr; // Required for H256::from_str, Address::from_str
        use ethers::core::types::{H256, Address, U256, U64}; // Import necessary types
        use sqlx::Row as SqlxRow; // Import Row and alias it to SqlxRow

        // Helper closures for parsing (can be defined once at module level or passed if preferred)
        let parse_h256_from_str = |s_opt: Option<String>| -> H256 {
            s_opt.and_then(|s| H256::from_str(&s).ok()).unwrap_or_default()
        };
        let parse_address_from_str = |s_opt: Option<String>| -> Address {
            s_opt.and_then(|s| Address::from_str(&s).ok()).unwrap_or_default()
        };
        let parse_option_address_from_str = |s_opt: Option<String>| -> Option<Address> {
            s_opt.and_then(|s| Address::from_str(&s).ok())
        };
        let parse_u256_from_text = |s_opt: Option<String>| -> U256 {
            s_opt.and_then(|s| U256::from_dec_str(&s).ok()).unwrap_or_default()
        };
        let parse_option_u256_from_text = |s_opt: Option<String>| -> Option<U256> {
            s_opt.and_then(|s| U256::from_dec_str(&s).ok())
        };

        // Map to MyTransaction struct
        // Assuming MyTransaction fields for numbers (block_number, transaction_index) are U64/Option<U64>
        let my_tx = MyTransaction {
            tx_hash: parse_h256_from_str(SqlxRow::try_get(&row, "tx_hash").ok()),
            block_number: U64::from(SqlxRow::try_get::<i64, _>(&row, "block_number").unwrap_or_default()),
            block_hash: parse_h256_from_str(SqlxRow::try_get(&row, "block_hash").ok()),
            transaction_index: SqlxRow::try_get::<Option<i64>, _>(&row, "transaction_index").ok().flatten().map(U64::from),
            from_address: parse_address_from_str(SqlxRow::try_get(&row, "from_address").ok()),
            to_address: parse_option_address_from_str(SqlxRow::try_get(&row, "to_address").ok()),
            value: parse_u256_from_text(SqlxRow::try_get(&row, "value").ok()),
            gas_price: parse_option_u256_from_text(SqlxRow::try_get(&row, "gas_price").ok()),
            max_fee_per_gas: parse_option_u256_from_text(SqlxRow::try_get(&row, "max_fee_per_gas").ok()),
            max_priority_fee_per_gas: parse_option_u256_from_text(SqlxRow::try_get(&row, "max_priority_fee_per_gas").ok()),
            gas: parse_u256_from_text(SqlxRow::try_get(&row, "gas_provided").ok()), // Ensure column name "gas_provided" matches table
            input_data: SqlxRow::try_get(&row, "input_data").unwrap_or_default(),
            status: SqlxRow::try_get::<Option<i16>, _>(&row, "status").ok().flatten().map(|s| s as u64),
        };
        Ok(Json(my_tx))
    } else {
        Err(ApiError::NotFound(format!(
            "Transaction {} not found",
            tx_hash_param // Use original param for error message
        )))
    }
}

pub(crate) async fn run_api_server(pool: PgPool) -> eyre::Result<()> {
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/logs", post(get_logs_handler))
        .route("/block/{block_number}", get(get_block_by_number_handler))
        .route(
            "/transaction/{tx_hash}",
            get(get_transaction_by_hash_handler),
        ) // **** NEW ROUTE ****
        .with_state(pool.clone());

    // ... rest of the function ...
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("API server listening on http://{}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
        .await
        .map_err(|e| eyre::eyre!("API server error: {}", e))?;

    Ok(())
}
