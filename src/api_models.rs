use serde::{ Deserialize};

#[derive(Debug, Deserialize)]
pub struct GetLogsFilter {
    #[serde(rename = "fromBlock")] // Maps JSON "fromBlock" to this field
    pub from_block: Option<u64>,
    #[serde(rename = "toBlock")]
    pub to_block: Option<u64>,
    pub address: Option<String>, // Contract address as a hex string
    // For simplicity, we'll just handle topic0 for now
    // Later, topics could be Vec<Option<String>> or more complex
    pub topic0: Option<String>,  // A single topic hash as a hex string
}