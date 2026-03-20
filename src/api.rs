// src/api.rs

// --- Imports for Swagger/OpenAPI Documentation ---
use crate::api_models::GenericErrorResponse;
use crate::docs::ApiDoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

// --- Imports for Axum and Business Logic ---
use crate::{
    api_models::{GetLogsFilter, IndexerStats, LogsResponse},
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
                tracing::error!("Database error: {:?}", db_err);
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
/// Retrieves a paginated list of event logs. Supports both offset pagination (page/page_size)
/// and stable cursor-based pagination (cursor_block + cursor_log_id from a previous response).
/// Cursor-based pagination is preferred at scale — O(log n) vs OFFSET's O(n) full scan.
#[utoipa::path(
    post,
    path = "/logs",
    request_body = GetLogsFilter,
    responses(
        (status = 200, description = "Successfully retrieved logs", body = LogsResponse),
        (status = 400, description = "Bad request due to invalid filters", body = GenericErrorResponse),
        (status = 500, description = "Internal server error", body = GenericErrorResponse),
    )
)]
async fn get_logs_handler(
    State(pool): State<PgPool>,
    Json(filters): Json<GetLogsFilter>,
) -> Result<Json<LogsResponse>, ApiError> {
    let page_size = filters.page_size.clamp(1, MAX_PAGE_SIZE);
    let use_cursor = filters.cursor_block.is_some() || filters.cursor_log_id.is_some();

    let mut query_builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, log_index_in_tx AS log_index, transaction_hash, \
         transaction_index_in_block AS transaction_index, \
         block_number, block_hash, contract_address AS address, \
         ENCODE(data, 'escape') AS data, all_topics AS topics \
         FROM logs WHERE 1=1",
    );

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
        query_builder.push(" AND LOWER(contract_address) = LOWER(");
        query_builder.push_bind(addr_filter);
        query_builder.push(")");
    }
    if let Some(t) = &filters.topic0 {
        query_builder.push(" AND topic0 = ");
        query_builder.push_bind(t);
    }
    if let Some(t) = &filters.topic1 {
        query_builder.push(" AND topic1 = ");
        query_builder.push_bind(t);
    }
    if let Some(t) = &filters.topic2 {
        query_builder.push(" AND topic2 = ");
        query_builder.push_bind(t);
    }
    if let Some(t) = &filters.topic3 {
        query_builder.push(" AND topic3 = ");
        query_builder.push_bind(t);
    }

    // Cursor: WHERE (block_number, id) > ($cursor_block, $cursor_log_id)
    // Deterministic ordering, no duplicate/skipped rows, O(log n) with composite index.
    if use_cursor {
        let cb = filters.cursor_block.unwrap_or(0);
        let cl = filters.cursor_log_id.unwrap_or(0);
        query_builder.push(" AND (block_number, id) > (");
        query_builder.push_bind(cb);
        query_builder.push(", ");
        query_builder.push_bind(cl);
        query_builder.push(")");
    }

    query_builder.push(" ORDER BY block_number ASC, id ASC LIMIT ");
    query_builder.push_bind(page_size as i64);

    if !use_cursor {
        let page = filters.page.max(1);
        let offset = (page - 1) * page_size;
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset as i64);
    }

    let rows = query_builder.build().fetch_all(&pool).await?;

    let mut next_cursor_block: Option<i64> = None;
    let mut next_cursor_log_id: Option<i64> = None;

    let logs = rows
        .into_iter()
        .map(|row| -> Result<MyLog, ApiError> {
            let row_id: i64 = SqlxRow::try_get(&row, "id")?;
            let block_num: i64 = SqlxRow::try_get(&row, "block_number")?;
            next_cursor_block = Some(block_num);
            next_cursor_log_id = Some(row_id);

            Ok(MyLog {
                log_index: SqlxRow::try_get::<Option<i64>, _>(&row, "log_index")?
                    .and_then(|v| U256::from_dec_str(&v.to_string()).ok()),
                transaction_hash: H256::from_str(&SqlxRow::try_get::<String, _>(
                    &row,
                    "transaction_hash",
                )?)
                .map_err(|e| {
                    ApiError::InternalServerError(format!("Invalid transaction_hash: {}", e))
                })?,
                transaction_index: SqlxRow::try_get::<Option<i64>, _>(&row, "transaction_index")?
                    .map(|v| v as u64),
                block_number: block_num as u64,
                block_hash: H256::from_str(&SqlxRow::try_get::<String, _>(&row, "block_hash")?)
                    .map_err(|e| {
                        ApiError::InternalServerError(format!("Invalid block_hash: {}", e))
                    })?,
                address: Address::from_str(&SqlxRow::try_get::<String, _>(&row, "address")?)
                    .map_err(|e| {
                        ApiError::InternalServerError(format!("Invalid address: {}", e))
                    })?,
                data: SqlxRow::try_get::<Option<String>, _>(&row, "data")?.unwrap_or_default(),
                topics: SqlxRow::try_get(&row, "topics").unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<MyLog>, ApiError>>()?;

    Ok(Json(LogsResponse {
        logs,
        next_cursor_block,
        next_cursor_log_id,
    }))
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

    // Fetch last_processed_block and chain_head for lag calculation
    let status = crate::db::get_indexer_status(&pool).await?;
    let (last_synced_block, ingestion_lag) = match status {
        Some((last, head)) => (Some(last), Some(head - last)),
        None => (None, None),
    };

    Ok(Json(IndexerStats {
        total_blocks: total_blocks.0,
        total_transactions: total_transactions.0,
        total_logs: total_logs.0,
        last_synced_block,
        ingestion_lag,
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
