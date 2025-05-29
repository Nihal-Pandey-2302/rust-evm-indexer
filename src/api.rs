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

const MAX_PAGE_SIZE: u64 = 100;

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    InternalServerError(String),
    DatabaseError(sqlx::Error),
    BadRequest(String), // <-- NEW VARIANT for client errors
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            ApiError::InternalServerError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
            ApiError::DatabaseError(db_err) => {
                eprintln!("Database error: {:?}", db_err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "A database error occurred".to_string(),
                )
            }
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message), // <-- HANDLE NEW VARIANT
        };

        let body = Json(json!({
            "status": if status.is_client_error() { "fail" } else { "error" }, // "fail" for 4xx, "error" for 5xx
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
    Json(filters): Json<GetLogsFilter>, // filters.page and filters.page_size now have defaults
) -> Result<Json<Vec<MyLog>>, ApiError> {
    println!("Received /logs request with filters: {:?}", filters);

    // --- Pagination Logic ---
    // filters.page and filters.page_size have defaults from GetLogsFilter struct
    let page = filters.page.max(1); // Ensure page is at least 1
    let page_size = filters.page_size.min(MAX_PAGE_SIZE).max(1); // Cap page_size and ensure it's at least 1
    let offset = (page - 1) * page_size;

    // Start building the query
    let mut query_builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, log_index_in_tx, transaction_hash, transaction_index_in_block, \
         block_number, block_hash, contract_address, data, \
         topic0, topic1, topic2, topic3, all_topics \
         FROM logs",
    );
    query_builder.push(" WHERE 1=1"); // Base condition for easier AND appending

    // --- Filter Logic ---
    // Handle blockHash first, as it overrides fromBlock/toBlock
    if let Some(bh_filter) = &filters.block_hash {
        query_builder.push(" AND LOWER(block_hash) = LOWER(");
        query_builder.push_bind(bh_filter.clone());
        query_builder.push(")");
    } else {
        // Only apply fromBlock and toBlock if blockHash is not present
        if let Some(fb) = filters.from_block {
            query_builder.push(" AND block_number >= ");
            query_builder.push_bind(fb as i64);
        }
        if let Some(tb) = filters.to_block {
            query_builder.push(" AND block_number <= ");
            query_builder.push_bind(tb as i64);
        }
    }

    // Address filter
    if let Some(addr_filter) = &filters.address {
        query_builder.push(" AND LOWER(contract_address) = LOWER(");
        query_builder.push_bind(addr_filter.clone());
        query_builder.push(")");
    }

    // Topic filters
    if let Some(topic0_filter) = &filters.topic0 {
        query_builder.push(" AND LOWER(topic0) = LOWER(");
        query_builder.push_bind(topic0_filter.clone());
        query_builder.push(")");
    }
    if let Some(topic1_filter) = &filters.topic1 {
        query_builder.push(" AND LOWER(topic1) = LOWER(");
        query_builder.push_bind(topic1_filter.clone());
        query_builder.push(")");
    }
    if let Some(topic2_filter) = &filters.topic2 {
        query_builder.push(" AND LOWER(topic2) = LOWER(");
        query_builder.push_bind(topic2_filter.clone());
        query_builder.push(")");
    }
    if let Some(topic3_filter) = &filters.topic3 {
        query_builder.push(" AND LOWER(topic3) = LOWER(");
        query_builder.push_bind(topic3_filter.clone());
        query_builder.push(")");
    }

    // **** REMOVED duplicated fromBlock/toBlock filters from here ****

    // --- Apply Ordering and Pagination to QueryBuilder ---
    query_builder
        .push(" ORDER BY block_number ASC, transaction_index_in_block ASC, log_index_in_tx ASC"); // Keep ordering consistent
    query_builder.push(" LIMIT ");
    query_builder.push_bind(page_size as i64); // SQL LIMIT
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset as i64); // SQL OFFSET

    let query = query_builder.build();
    println!("Executing SQL: {}", query.sql()); // This will now include LIMIT and OFFSET placeholders

    let rows = query.fetch_all(&pool).await?;

    let mut logs_result: Vec<MyLog> = Vec::new();
    // Ensure these use statements are available for the mapping logic
    use ethers::core::types::{Address, H256, U256}; // U64 might not be needed if not in MyLog here
    use sqlx::Row as SqlxRow;
    use std::str::FromStr;

    for row in rows {
        // Helper closures for parsing (defined here for clarity or move to module level)
        let parse_h256 = |s: Option<String>| -> H256 {
            s.and_then(|str_val| H256::from_str(&str_val).ok())
                .unwrap_or_default()
        };
        let parse_address = |s: Option<String>| -> Address {
            s.and_then(|str_val| Address::from_str(&str_val).ok())
                .unwrap_or_default()
        };

        let topics_from_db: Vec<String> = SqlxRow::try_get(&row, "all_topics").unwrap_or_default();

        let log_entry = MyLog {
            log_index: SqlxRow::try_get::<Option<i64>, _>(&row, "log_index_in_tx")
                .ok()
                .flatten()
                .map(U256::from),
            transaction_hash: parse_h256(SqlxRow::try_get(&row, "transaction_hash").ok()),
            transaction_index: SqlxRow::try_get::<Option<i64>, _>(
                &row,
                "transaction_index_in_block",
            )
            .ok()
            .flatten()
            .map(|v| v as u64), // Assuming MyLog.transaction_index is Option<u64>
            block_number: SqlxRow::try_get::<i64, _>(&row, "block_number")
                .map(|v| v as u64)
                .unwrap_or(0), // Assuming MyLog.block_number is u64
            block_hash: parse_h256(SqlxRow::try_get(&row, "block_hash").ok()),
            address: parse_address(SqlxRow::try_get(&row, "contract_address").ok()),
            data: SqlxRow::try_get(&row, "data").unwrap_or_else(|_| String::from("0x")),
            topics: topics_from_db,
        };
        logs_result.push(log_entry);
    }
    Ok(Json(logs_result))
}

pub async fn get_block_handler(
    // Renamed function
    State(pool): State<PgPool>,
    Path(identifier): Path<String>, // Accepts a String identifier
) -> Result<Json<MyBlock>, ApiError> {
    println!("Received /block/{} request", identifier);

    let row_option: Option<sqlx::postgres::PgRow>; // Declare row_option outside the if/else

    if identifier.starts_with("0x") && identifier.len() == 66 {
        // Assume it's a block hash
        // It's good practice to ensure the hash is lowercase for consistent querying if your DB stores it that way.
        let block_hash_to_query = identifier.to_lowercase();
        println!("Attempting to fetch block by hash: {}", block_hash_to_query);
        row_option = sqlx::query(
            "SELECT block_number, block_hash, parent_hash, timestamp, gas_used, gas_limit, base_fee_per_gas \
             FROM blocks WHERE block_hash = $1" // Query by block_hash
            )
            .bind(block_hash_to_query) // Bind the block hash string
            .fetch_optional(&pool)
            .await?; // The ? uses From<sqlx::Error> for ApiError
    } else if let Ok(block_number_param) = identifier.parse::<u64>() {
        // Assume it's a block number
        println!(
            "Attempting to fetch block by number: {}",
            block_number_param
        );
        let block_number_db = block_number_param as i64;
        row_option = sqlx::query(
            "SELECT block_number, block_hash, parent_hash, timestamp, gas_used, gas_limit, base_fee_per_gas \
             FROM blocks WHERE block_number = $1" // Query by block_number
            )
            .bind(block_number_db)
            .fetch_optional(&pool)
            .await?; // The ? uses From<sqlx::Error> for ApiError
    } else {
        // Invalid identifier format
        return Err(ApiError::BadRequest(format!(
            "Invalid block identifier format: {}. Must be a block number or a 0x-prefixed 66-character hash.",
            identifier
        )));
    }

    if let Some(row) = row_option {
        // Helper closures for parsing (can be defined once at module level or passed if preferred)
        // Ensure these `use` statements are within a scope that covers these closures if defined here,
        // or at the top of the file if helpers are module-level functions.
        // For clarity, if these are only used here, keeping them local is fine.
        use std::str::FromStr; // Already likely at top of api.rs
        use ethers::core::types::{H256, U256, U64};
        use sqlx::Row as SqlxRow; // Already likely at top of api.rs

        let parse_h256_from_str = |s: Option<String>| -> H256 {
            s.and_then(|str_val| H256::from_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_u256_from_text = |s: Option<String>| -> U256 {
            s.and_then(|str_val| U256::from_dec_str(&str_val).ok()).unwrap_or_default()
        };
        let parse_option_u256_from_text = |s: Option<String>| -> Option<U256> {
            s.and_then(|str_val| U256::from_dec_str(&str_val).ok())
        };

        // Map to MyBlock struct
        // Assuming MyBlock fields are defined with ethers-rs types (U64, U256, H256)
        let my_block = MyBlock {
            block_number: U64::from(SqlxRow::try_get::<i64, _>(&row, "block_number").unwrap_or_default()),
            block_hash: parse_h256_from_str(SqlxRow::try_get(&row, "block_hash").ok()),
            parent_hash: parse_h256_from_str(SqlxRow::try_get(&row, "parent_hash").ok()),
            timestamp: U256::from(SqlxRow::try_get::<i64, _>(&row, "timestamp").unwrap_or_default()),
            gas_used: parse_u256_from_text(SqlxRow::try_get(&row, "gas_used").ok()),
            gas_limit: parse_u256_from_text(SqlxRow::try_get(&row, "gas_limit").ok()),
            base_fee_per_gas: parse_option_u256_from_text(SqlxRow::try_get(&row, "base_fee_per_gas").ok()),
        };
        Ok(Json(my_block))
    } else {
        Err(ApiError::NotFound(format!(
            "Block with identifier '{}' not found",
            identifier
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
        .route("/block/{block_number}", get(get_block_handler))
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
