use dotenvy::dotenv; // Import the dotenv function
use ethers::providers::{Http, Middleware, Provider};
use eyre::Result;
use std::env; // To read environment variables

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenv().ok(); // .ok() converts Result to Option, effectively ignoring if .env is not found (e.g., in production)

    println!("Attempting to connect to Ethereum node...");

    // Read the RPC URL from an environment variable
    let rpc_url = env::var("ETH_RPC_URL")
        .map_err(|e| eyre::eyre!("ETH_RPC_URL not found in environment: {}", e))?;

    println!("Using RPC URL: {}", rpc_url); // Be careful about logging sensitive URLs in production logs

    // Create a provider
    let provider = Provider::<Http>::try_from(rpc_url.as_str()) // Use rpc_url.as_str() as try_from expects &str
        .map_err(|e| eyre::eyre!("Failed to create provider: {}", e))?;

    println!("Successfully connected to provider.");

    // Get the current block number
    let block_number = provider.get_block_number().await
        .map_err(|e| eyre::eyre!("Failed to get block number: {}", e))?;

    println!("Current Ethereum block number: {}", block_number);

    Ok(())
}