// src/models.rs

use ethers::core::types::{Address, H256, U256, U64};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema; // Import ToSchema
use sqlx::FromRow;

// --- Annotate each struct with ToSchema and its fields ---

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")] // Good practice for JSON APIs
pub struct MyLog {
    #[schema(value_type = Option<String>, example = "0x1")]
    pub log_index: Option<U256>,
    #[schema(value_type = String, example = "0x...")]
    pub transaction_hash: H256,
    pub transaction_index: Option<u64>,
    pub block_number: u64,
    #[schema(value_type = String, example = "0x...")]
    pub block_hash: H256,
    #[schema(value_type = String, example = "0x...")]
    pub address: Address,
    #[schema(example = "0x0000...")]
    pub data: String,
    #[schema(example = json!(["0x...", "0x..."]))]
    pub topics: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyBlock {
    #[schema(value_type = u64, example = 18000000)]
    pub block_number: U64,
    #[schema(value_type = String, example = "0x...")]
    pub block_hash: H256,
    #[schema(value_type = String, example = "0x...")]
    pub parent_hash: H256,
    #[schema(value_type = u64, example = 1694035835)]
    pub timestamp: U256,
    #[schema(value_type = String, example = "15000000")]
    pub gas_used: U256,
    #[schema(value_type = String, example = "30000000")]
    pub gas_limit: U256,
    #[schema(value_type = Option<String>, example = "20.123456789")]
    pub base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyTransaction {
    #[schema(value_type = String, example = "0x...")]
    pub tx_hash: H256,
    #[schema(value_type = u64, example = 18000000)]
    pub block_number: U64,
    #[schema(value_type = String, example = "0x...")]
    pub block_hash: H256,
    #[schema(value_type = Option<u64>, example = 100)]
    pub transaction_index: Option<U64>,
    #[schema(value_type = String, example = "0x...")]
    pub from_address: Address,
    #[schema(value_type = Option<String>, example = "0x...")]
    pub to_address: Option<Address>,
    #[schema(value_type = String, example = "1000000000000000000")] // 1 ETH
    pub value: U256,
    #[schema(value_type = Option<String>, example = "25000000000")] // 25 Gwei
    pub gas_price: Option<U256>,
    #[schema(value_type = Option<String>)]
    pub max_fee_per_gas: Option<U256>,
    #[schema(value_type = Option<String>)]
    pub max_priority_fee_per_gas: Option<U256>,
    #[schema(value_type = String, example = "21000")]
    pub gas: U256,
    #[schema(example = "0x...")]
    pub input_data: String,
    pub status: Option<u64>,
}