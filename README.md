# EVM Indexer in Rust 🦀

A high-performance Ethereum Virtual Machine (EVM) historical data ingester and query API, built with Rust. This project focuses on ingesting blocks, transactions, and logs from an Ethereum node, storing them efficiently, and providing a queryable API.

## 🌟 Project Goals & Motivation

* To gain practical experience in building robust systems with Rust.
* To deepen understanding of Ethereum's data structures, JSON-RPC API, and EVM internals.
* To explore efficient data ingestion, storage (initially PostgreSQL), and API design patterns.
* To serve as a significant learning tool and portfolio piece for blockchain protocol engineering.
* [Add any other personal motivations or learning goals]

## ✨ Features

* **Data Ingestion:**
  * [x] Connect to an Ethereum node and fetch the current block number.
  * [ ] Fetch historical block data (headers, full transaction objects) for a range of blocks.
  * [ ] Extract transactions from blocks.
  * [ ] Extract event logs from transaction receipts.
* **Storage:**
  * [ ] Store ingested data in a PostgreSQL database.
  * [ ] Design an efficient and scalable database schema.
* **API:**
  * [ ] Develop a REST API (using Axum/Actix-Web or similar).
  * [ ] Implement a `getLogs` endpoint mimicking standard JSON-RPC functionality.
* **Core:**
  * Built with Rust for performance and safety.
  * [x] Asynchronous processing using Tokio.
  * [x] Interaction with Ethereum nodes via `ethers-rs`.

## 🛠️ Tech Stack

* **Language:** Rust (Edition 2021)
* **Async Runtime:** Tokio
* **Ethereum Interaction:** `ethers-rs`
* **Configuration:** `dotenvy` (for environment variables)
* **Database:** PostgreSQL (Planned - using `sqlx`)
* **API Framework:** Axum (Planned)
* **Serialization:** `serde`

## 🚀 Getting Started

### Prerequisites

* Rust toolchain (visit [rustup.rs](https://rustup.rs/))
* Access to an Ethereum JSON-RPC endpoint (e.g., from Infura, Alchemy, or a local node)

### Installation & Running

1. **Clone the repository:**

    ```bash
    git clone https://github.com/Nihal-Pandey-2302/rust-evm-indexer.git
    cd rust-evm-indexer
    ```

2. **Set up your environment variables:**
    This project uses a `.env` file to manage the Ethereum RPC URL. Create a file named `.env` in the root of the project directory and add your RPC URL like this:

    ```env
    # .env
    ETH_RPC_URL=YOUR_ETHEREUM_NODE_RPC_URL_HERE
    ```
    Replace `YOUR_ETHEREUM_NODE_RPC_URL_HERE` with your actual Ethereum JSON-RPC endpoint.
    **Note:** The `.env` file is included in `.gitignore` and should not be committed to the repository.

3.  **Build the project:**
    ```bash
    cargo build
    ```

4. **Run the project:**

    ```bash
    cargo run
    ```

    This will load the `ETH_RPC_URL` from your `.env` file, attempt to connect to the Ethereum node, and print the latest block number.

## 🗺️ Project Status & Roadmap

* **Current Status:** Initial project setup complete. Able to connect to an Ethereum node via `ethers-rs` and Tokio to fetch and display the current block number.
* **Next Steps:**
    1. Implement a robust loop for fetching historical block data (headers and transactions).
    2. Define the database schema for blocks, transactions, and event logs.
    3. Integrate PostgreSQL using `sqlx` for data storage.
    4. Begin development of the `getLogs` API endpoint.



## 📜 License

This project is licensed under the [MIT License](LICENSE).

