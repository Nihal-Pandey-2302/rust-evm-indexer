// src/api.rs

// --- Imports for Swagger/OpenAPI Documentation ---
use crate::api_models::GenericErrorResponse;
use crate::docs::ApiDoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

// --- Imports for Axum and Business Logic ---
use crate::{
    api_models::{GetLogsFilter, IndexerStats},
    models::{MyBlock, MyLog, MyTransaction},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use ethers::core::types::{Address, H256, U256, U64};
use sqlx::{PgPool, QueryBuilder, Row as SqlxRow};
use std::net::SocketAddr;
use std::str::FromStr;

const MAX_PAGE_SIZE: u64 = 100;

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    InternalServerError(String),
    DatabaseError(sqlx::Error),
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::DatabaseError(db_err) => {
                eprintln!("Database error: {:?}", db_err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "A database error occurred".to_string(),
                )
            }
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = GenericErrorResponse {
            status: if status.is_client_error() {
                "fail".to_string()
            } else {
                "error".to_string()
            },
            status_code: status.as_u16(),
            message,
        };

        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => {
                ApiError::NotFound("The requested resource was not found.".to_string())
            }
            _ => ApiError::DatabaseError(err),
        }
    }
}

impl From<eyre::Report> for ApiError {
    fn from(err: eyre::Report) -> Self {
        ApiError::InternalServerError(err.to_string())
    }
}

/// API Root
///
/// Provides a simple welcome message to verify the API is running.
#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Success", body = String, content_type = "text/html")
    )
)]
pub async fn root_handler() -> Html<&'static str> {
    Html("<h1>Hello, EVM Indexer API!</h1><p>Welcome to your Rust-powered API.</p>")
}

/// Get Filtered Logs
///
/// Retrieves a paginated list of event logs based on a set of filters provided in the request body.
#[utoipa::path(
    post,
    path = "/logs",
    request_body = GetLogsFilter,
    responses(
        (status = 200, description = "Successfully retrieved logs", body = [MyLog]),
        (status = 400, description = "Bad request due to invalid filters", body = GenericErrorResponse),
        (status = 500, description = "Internal server error", body = GenericErrorResponse),
    )
)]
async fn get_logs_handler(
    State(pool): State<PgPool>,
    Json(filters): Json<GetLogsFilter>,
) -> Result<Json<Vec<MyLog>>, ApiError> {
    let page = filters.page.max(1);
    let page_size = filters.page_size.clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * page_size;

    let mut query_builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT log_index, transaction_hash, transaction_index, \
         block_number, block_hash, address, data, topics \
         FROM logs",
    );
    query_builder.push(" WHERE 1=1");

    // --- FIX: Restore full filter logic to resolve warnings ---
    if let Some(bh_filter) = &filters.block_hash {
        query_builder.push(" AND LOWER(block_hash) = LOWER(");
        query_builder.push_bind(bh_filter);
        query_builder.push(")");
    } else {
        if let Some(fb) = filters.from_block {
            query_builder.push(" AND block_number >= ");
            query_builder.push_bind(fb as i64);
        }
        if let Some(tb) = filters.to_block {
            query_builder.push(" AND block_number <= ");
            query_builder.push_bind(tb as i64);
        }
    }
    if let Some(addr_filter) = &filters.address {
        query_builder.push(" AND LOWER(address) = LOWER(");
        query_builder.push_bind(addr_filter);
        query_builder.push(")");
    }
    // This assumes your DB schema has separate columns topic0, topic1, etc.
    // If you only have a `topics` array, the query would need to be different.
    if let Some(topic0_filter) = &filters.topic0 {
        query_builder.push(" AND topics[1] = "); // PG arrays are 1-indexed
        query_builder.push_bind(topic0_filter);
    }
    if let Some(topic1_filter) = &filters.topic1 {
        query_builder.push(" AND topics[2] = ");
        query_builder.push_bind(topic1_filter);
    }
    if let Some(topic2_filter) = &filters.topic2 {
        query_builder.push(" AND topics[3] = ");
        query_builder.push_bind(topic2_filter);
    }
    if let Some(topic3_filter) = &filters.topic3 {
        query_builder.push(" AND topics[4] = ");
        query_builder.push_bind(topic3_filter);
    }

    query_builder.push(" ORDER BY block_number ASC, transaction_index ASC, log_index ASC");
    query_builder.push(" LIMIT ");
    query_builder.push_bind(page_size as i64);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset as i64);

    let rows = query_builder.build().fetch_all(&pool).await?;

    let logs_result = rows
        .into_iter()
        .map(|row| -> Result<MyLog, ApiError> {
            Ok(MyLog {
                log_index: SqlxRow::try_get::<Option<String>, _>(&row, "log_index")?
                    .and_then(|s| U256::from_dec_str(&s).ok()),
                transaction_hash: H256::from_str(&SqlxRow::try_get::<String, _>(
                    &row,
                    "transaction_hash",
                )?)
                .map_err(|e| {
                    ApiError::InternalServerError(format!("Invalid transaction_hash format: {}", e))
                })?,
                transaction_index: SqlxRow::try_get::<Option<i64>, _>(&row, "transaction_index")?
                    .map(|v| v as u64),
                block_number: SqlxRow::try_get::<i64, _>(&row, "block_number")? as u64,
                block_hash: H256::from_str(&SqlxRow::try_get::<String, _>(&row, "block_hash")?)
                    .map_err(|e| {
                        ApiError::InternalServerError(format!("Invalid block_hash format: {}", e))
                    })?,
                address: Address::from_str(&SqlxRow::try_get::<String, _>(&row, "address")?)
                    .map_err(|e| {
                        ApiError::InternalServerError(format!("Invalid address format: {}", e))
                    })?,
                data: SqlxRow::try_get(&row, "data")?,
                topics: SqlxRow::try_get(&row, "topics")?,
            })
        })
        .collect::<Result<Vec<MyLog>, ApiError>>()?;

    Ok(Json(logs_result))
}

/// Get Indexer Stats
///
/// Retrieves overall statistics for the indexer including total blocks, transactions, logs, and the last synced block.
#[utoipa::path(
    get,
    path = "/stats",
    responses(
        (status = 200, description = "Indexer stats retrieved successfully", body = IndexerStats),
        (status = 500, description = "Internal server error", body = GenericErrorResponse)
    )
)]
pub async fn get_stats_handler(State(pool): State<PgPool>) -> Result<Json<IndexerStats>, ApiError> {
    let total_blocks: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blocks")
        .fetch_one(&pool)
        .await?;
    let total_transactions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await?;
    let total_logs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM logs")
        .fetch_one(&pool)
        .await?;

    let last_synced_block: Option<(i64,)> = sqlx::query_as(
        "SELECT last_processed_block FROM indexer_status WHERE indexer_name = 'evm_main_sync'",
    )
    .fetch_optional(&pool)
    .await?;

    Ok(Json(IndexerStats {
        total_blocks: total_blocks.0,
        total_transactions: total_transactions.0,
        total_logs: total_logs.0,
        last_synced_block: last_synced_block.map(|r| r.0),
    }))
}

/// Get Block by Number or Hash
///
/// Retrieves a full block by its number or 0x-prefixed hash.
#[utoipa::path(
    get,
    path = "/block/{identifier}",
    params(
        ("identifier" = String, Path, description = "Block number or hash", example = "18000000")
    ),
    responses(
        (status = 200, description = "Block found", body = MyBlock),
        (status = 404, description = "Block not found", body = GenericErrorResponse),
        (status = 400, description = "Invalid identifier format", body = GenericErrorResponse)
    )
)]
pub async fn get_block_handler(
    State(pool): State<PgPool>,
    Path(identifier): Path<String>,
) -> Result<Json<MyBlock>, ApiError> {
    let query = "SELECT block_number, block_hash, parent_hash, timestamp, gas_used, gas_limit, base_fee_per_gas FROM blocks";

    let row = if identifier.starts_with("0x") {
        sqlx::query(&format!("{} WHERE block_hash = $1", query))
            .bind(identifier.to_lowercase())
            .fetch_one(&pool)
            .await?
    } else {
        let block_number = identifier
            .parse::<i64>()
            .map_err(|_| ApiError::BadRequest("Invalid block number format".to_string()))?;
        sqlx::query(&format!("{} WHERE block_number = $1", query))
            .bind(block_number)
            .fetch_one(&pool)
            .await?
    };

    let my_block = MyBlock {
        block_number: U64::from(SqlxRow::try_get::<i64, _>(&row, "block_number")?),
        block_hash: H256::from_str(&SqlxRow::try_get::<String, _>(&row, "block_hash")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid block_hash: {}", e)))?,
        parent_hash: H256::from_str(&SqlxRow::try_get::<String, _>(&row, "parent_hash")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid parent_hash: {}", e)))?,
        timestamp: U256::from(SqlxRow::try_get::<i64, _>(&row, "timestamp")?),
        gas_used: U256::from_dec_str(&SqlxRow::try_get::<String, _>(&row, "gas_used")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid gas_used: {}", e)))?,
        gas_limit: U256::from_dec_str(&SqlxRow::try_get::<String, _>(&row, "gas_limit")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid gas_limit: {}", e)))?,
        base_fee_per_gas: SqlxRow::try_get::<Option<String>, _>(&row, "base_fee_per_gas")?
            .and_then(|s| U256::from_dec_str(&s).ok()),
    };

    Ok(Json(my_block))
}

/// Get Transaction by Hash
///
/// Retrieves a specific transaction by its 0x-prefixed hash.
#[utoipa::path(
    get,
    path = "/transaction/{tx_hash}",
    params(
        ("tx_hash" = String, Path, description = "The transaction hash", example = "0x...")
    ),
    responses(
        (status = 200, description = "Transaction found", body = MyTransaction),
        (status = 404, description = "Transaction not found", body = GenericErrorResponse),
        (status = 400, description = "Invalid hash format", body = GenericErrorResponse)
    )
)]
pub async fn get_transaction_by_hash_handler(
    State(pool): State<PgPool>,
    Path(tx_hash_param): Path<String>,
) -> Result<Json<MyTransaction>, ApiError> {
    if !tx_hash_param.starts_with("0x") || tx_hash_param.len() != 66 {
        return Err(ApiError::BadRequest(
            "Invalid transaction hash format.".to_string(),
        ));
    }

    let row = sqlx::query(
        "SELECT tx_hash, block_number, block_hash, transaction_index, \
         from_address, to_address, value, gas_price, max_fee_per_gas, \
         max_priority_fee_per_gas, gas_provided, input_data, status \
         FROM transactions WHERE tx_hash = $1",
    )
    .bind(tx_hash_param.to_lowercase())
    .fetch_one(&pool)
    .await?;

    let my_tx = MyTransaction {
        tx_hash: H256::from_str(&SqlxRow::try_get::<String, _>(&row, "tx_hash")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid tx_hash: {}", e)))?,
        block_number: U64::from(SqlxRow::try_get::<i64, _>(&row, "block_number")?),
        block_hash: H256::from_str(&SqlxRow::try_get::<String, _>(&row, "block_hash")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid block_hash: {}", e)))?,
        transaction_index: SqlxRow::try_get::<Option<i64>, _>(&row, "transaction_index")?
            .map(U64::from),
        from_address: Address::from_str(&SqlxRow::try_get::<String, _>(&row, "from_address")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid from_address: {}", e)))?,
        to_address: SqlxRow::try_get::<Option<String>, _>(&row, "to_address")?
            .and_then(|s| Address::from_str(&s).ok()),
        value: U256::from_dec_str(&SqlxRow::try_get::<String, _>(&row, "value")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid value: {}", e)))?,
        gas_price: SqlxRow::try_get::<Option<String>, _>(&row, "gas_price")?
            .and_then(|s| U256::from_dec_str(&s).ok()),
        max_fee_per_gas: SqlxRow::try_get::<Option<String>, _>(&row, "max_fee_per_gas")?
            .and_then(|s| U256::from_dec_str(&s).ok()),
        max_priority_fee_per_gas: SqlxRow::try_get::<Option<String>, _>(
            &row,
            "max_priority_fee_per_gas",
        )?
        .and_then(|s| U256::from_dec_str(&s).ok()),
        gas: U256::from_dec_str(&SqlxRow::try_get::<String, _>(&row, "gas_provided")?)
            .map_err(|e| ApiError::InternalServerError(format!("Invalid gas: {}", e)))?,
        input_data: SqlxRow::try_get(&row, "input_data")?,
        status: SqlxRow::try_get::<Option<i16>, _>(&row, "status")?.map(|s| s as u64),
    };

    Ok(Json(my_tx))
}

pub async fn run_api_server(pool: PgPool) -> eyre::Result<()> {
    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/", get(root_handler))
        .route("/stats", get(get_stats_handler))
        .route("/logs", post(get_logs_handler))
        // --- FIX: Use modern Axum path parameter syntax ---
        .route("/block/{identifier}", get(get_block_handler))
        .route(
            "/transaction/{tx_hash}",
            get(get_transaction_by_hash_handler),
        )
        .with_state(pool.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("API: Server listening on http://{}", addr);
    println!("API: View Swagger UI at http://{}/swagger-ui", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
        .await
        .map_err(|e| eyre::eyre!("API server error: {}", e))?;

    Ok(())
}
