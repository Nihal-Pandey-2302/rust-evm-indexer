// src/api_models.rs
use serde::Deserialize;

// Helper function to provide default for page
fn default_page() -> u64 {
    1 // Default to page 1
}

// Helper function to provide default for page_size
fn default_page_size() -> u64 {
    25 // Default to 25 items per page
}

#[derive(Debug, Deserialize)]
pub struct GetLogsFilter {
    #[serde(rename = "fromBlock")]
    pub from_block: Option<u64>,
    #[serde(rename = "toBlock")]
    pub to_block: Option<u64>,
    pub address: Option<String>,
    pub topic0: Option<String>,
    pub topic1: Option<String>,
    pub topic2: Option<String>,
    pub topic3: Option<String>,
    #[serde(rename = "blockHash")]
    pub block_hash: Option<String>,

    // --- NEW Pagination Fields ---
    // #[serde(default = "default_page")] will call default_page() if 'page' is missing or null
    #[serde(default = "default_page")]
    pub page: u64,

    // #[serde(default = "default_page_size")] for pageSize
    #[serde(default = "default_page_size", alias = "limit")] // alias "limit" for pageSize
    pub page_size: u64,
}
