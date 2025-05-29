// src/models.rs

use ethers::core::types::{Address, H256, U256, U64}; // Ensure U64 is imported
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MyLog {
    pub log_index: Option<U256>, // Log index within the block
    pub transaction_hash: H256,
    pub transaction_index: Option<u64>, // u64 from MyTransaction or Option<U256>
    pub block_number: u64,
    pub block_hash: H256,
    pub address: Address,    // Address of the contract that emitted the log
    pub data: String,        // Hex string of non-indexed log data
    pub topics: Vec<String>, // Vec of H256 topics, converted to hex strings
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MyBlock {
    pub block_number: U64, // CHANGE THIS to U64 if it was u64
    pub block_hash: H256,
    pub parent_hash: H256,
    pub timestamp: U256,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MyTransaction {
    pub tx_hash: H256,
    pub block_number: U64, // CHANGE THIS to U64 if it was u64
    pub block_hash: H256,
    pub transaction_index: Option<U64>, // Let's make this Option<U64> too for consistency
    pub from_address: Address,
    pub to_address: Option<Address>,
    pub value: U256,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub gas: U256,
    pub input_data: String,
    pub status: Option<u64>, // Status from receipt (0 or 1) can remain u64
}
