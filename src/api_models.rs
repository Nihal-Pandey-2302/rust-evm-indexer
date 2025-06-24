// src/api_models.rs
use serde::{Deserialize, Serialize}; // Add Serialize
use utoipa::{IntoParams, ToSchema}; // Import IntoParams and ToSchema

// Helper function to provide default for page
fn default_page() -> u64 {
    1
}

// Helper function to provide default for page_size
fn default_page_size() -> u64 {
    25
}

// NOTE: This struct is used as the REQUEST BODY for the POST /logs endpoint.
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GetLogsFilter {
    #[schema(example = 18000000)]
    pub from_block: Option<u64>,
    #[schema(example = 18000100)]
    pub to_block: Option<u64>,
    #[schema(example = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")]
    pub address: Option<String>,
    #[schema(example = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")]
    pub topic0: Option<String>,
    pub topic1: Option<String>,
    pub topic2: Option<String>,
    pub topic3: Option<String>,
    #[schema(example = "0x...")]
    pub block_hash: Option<String>,

    // Pagination Fields
    #[serde(default = "default_page")]
    #[param(example = 1)]
    pub page: u64,

    #[serde(default = "default_page_size", alias = "limit")]
    #[param(example = 25)]
    pub page_size: u64,
}

// A generic, serializable error response struct for consistent API errors.
#[derive(Serialize, ToSchema)]
pub struct GenericErrorResponse {
    pub status: String,
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    #[schema(example = "Resource not found")]
    pub message: String,
}