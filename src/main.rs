use ethers::providers::{Http, Middleware, Provider};
use eyre::Result; // Use eyre's Result for convenient error handling

// Replace this with your actual Ethereum node RPC URL
// You can get one from services like Infura, Alchemy, or use your own local node.
const RPC_URL: &str = "https://eth-mainnet.g.alchemy.com/v2/RdPcgIk1IKRRxNSsXc758zN7WgTCN0_9";

#[tokio::main] // This macro sets up the Tokio runtime for our async main function
async fn main() -> Result<()> {
    println!("Attempting to connect to Ethereum node at: {}", RPC_URL);

    // Create a provider
    let provider = Provider::<Http>::try_from(RPC_URL)
        .map_err(|e| eyre::eyre!("Failed to create provider: {}", e))?;
    // The .map_err part provides more context if Provider::try_from fails.

    println!("Successfully connected to provider.");

    // Get the current block number
    let block_number = provider.get_block_number().await
        .map_err(|e| eyre::eyre!("Failed to get block number: {}", e))?;

    println!("Current Ethereum block number: {}", block_number);

    Ok(())
}