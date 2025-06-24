// src/docs.rs
use crate::api_models::{GenericErrorResponse, GetLogsFilter};
use crate::models::{MyBlock, MyLog, MyTransaction};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::root_handler,
        crate::api::get_logs_handler,
        crate::api::get_block_handler,
        crate::api::get_transaction_by_hash_handler,
    ),
    components(
        schemas(
            // API Models
            GetLogsFilter,
            GenericErrorResponse,
            // Core DB Models
            MyBlock,
            MyTransaction,
            MyLog
        )
    ),
    tags(
        (name = "EVM Indexer API", description = "Endpoints for querying indexed blockchain data.")
    ),
    info(
        title = "EVM Indexer API",
        version = "1.0.0",
        description = "This API provides access to Ethereum blockchain data indexed by a custom Rust service. \
                       It allows querying for blocks, transactions, and logs with powerful filters.",
        contact(
            name = "Nihal Pandey",
            url = "https://github.com/Nihal-Pandey-2302/rust-evm-indexer"
        )
    )
)]
pub struct ApiDoc;