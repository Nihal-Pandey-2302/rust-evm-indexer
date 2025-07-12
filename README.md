# EVM Indexer in Rust 🦀

A high-performance Ethereum Virtual Machine (EVM) data indexer and query API, built with Rust. This project features a **continuously running ingester** that fetches blocks, transactions, and event logs from an Ethereum node, storing them in PostgreSQL. A **concurrent REST API**, complete with interactive Swagger UI documentation, provides queryable access to the indexed data.
<div align="center">
  <img src="https://github.com/Nihal-Pandey-2302/rust-evm-indexer/blob/main/evm-indexer.png" alt="EVM Indexer Architecture Diagram" width="700"/>
  <br/>
  <em>Figure 1: High-level architecture of the Rust-based EVM Indexer</em>
</div>

## 🌟 Project Goals & Motivation

* To gain practical experience in building robust systems with Rust.
* To deepen understanding of Ethereum's data structures, JSON-RPC API, and EVM internals.
* To explore efficient data ingestion (including continuous, stateful, and resilient syncing), storage (PostgreSQL with `sqlx`), and API design patterns (with `axum`).
* To serve as a significant learning tool and portfolio piece for blockchain protocol engineering.

## ✨ Features

* **Data Ingestion:**
    * [x] Fetch historical blocks, transactions, and event logs.
    * [x] Continuous polling for new blocks with state management to resume from the last sync point.
    * [x] Per-block data insertion within database transactions for atomicity.
    * [x] Retry logic with exponential backoff for critical RPC calls.
* **Storage:**
    * [x] Store ingested data in a PostgreSQL database with an optimized schema.
* **API (using Axum):**
    * [x] Concurrent REST API server.
    * [x] **Interactive API Documentation with Swagger UI.**
    * [x] Standardized JSON error handling.
    * [x] `POST /logs` endpoint with filtering (block range/hash, address, topics) and pagination.
    * [x] `GET /block/{identifier}` endpoint (accepts block number or hash).
    * [x] `GET /transaction/{transaction_hash}` endpoint.

## 🛠️ Tech Stack

* **Language:** Rust (Edition 2021)
* **Async Runtime:** Tokio
* **Ethereum Interaction:** `ethers-rs`
* **Database:** PostgreSQL (using `sqlx`)
* **API Framework:** Axum
* **API Documentation:** `utoipa` (for OpenAPI spec generation) & `utoipa-swagger-ui`
* **Configuration:** `dotenvy`
* **Serialization:** `serde`

## 🚀 Getting Started

### Prerequisites

* Rust toolchain (visit [rustup.rs](https://rustup.rs/))
* PostgreSQL server installed and running.
* Access to an Ethereum JSON-RPC endpoint (e.g., from Infura, Alchemy, or a local node).

### Installation & Running

1.  **Clone the repository:**
    ```bash
    git clone [https://github.com/Nihal-Pandey-2302/rust-evm-indexer.git](https://github.com/Nihal-Pandey-2302/rust-evm-indexer.git)
    cd rust-evm-indexer
    ```

2.  **Set up PostgreSQL Database & User:**
    ```bash
    sudo -u postgres psql
    ```sql
    CREATE USER indexer_user WITH PASSWORD 'YOUR_CHOSEN_PASSWORD';
    CREATE DATABASE evm_data_indexer OWNER indexer_user;
    \q
    ```

3.  **Set up your environment variables:**
    Create a `.env` file in the root directory and add the following:
    ```env
    # .env
    ETH_RPC_URL=YOUR_ETHEREUM_NODE_RPC_URL_HERE
    DATABASE_URL=postgres://indexer_user:YOUR_CHOSEN_PASSWORD@localhost:5432/evm_data_indexer
    ```

4.  **Create Database Tables:**
    Connect to your database (`psql -U indexer_user -d evm_data_indexer -h localhost`) and execute the table creation SQL found in the `schema.sql` file (or from the previous README version).

5.  **Build and Run the Project:**
    ```bash
    cargo run
    ```
    This command starts both the data ingester and the API server. The API server listens on `http://127.0.0.1:3000`. To stop both, press `Ctrl+C`.

### Accessing the API Documentation

Once the server is running, you can access the live, interactive Swagger UI documentation in your browser:

**Navigate to: [http://127.0.0.1:3000/swagger-ui](http://127.0.0.1:3000/swagger-ui)**

You can explore all available endpoints, see their request/response models, and execute API calls directly from the documentation page.

![Swagger UI Preview](https://github.com/Nihal-Pandey-2302/rust-evm-indexer/blob/main/assets/Swagger%20UI.png)

## 🗺️ Project Status & Roadmap

* **Current Status:**
    * **Concurrent Operation:** Ingester and API server run concurrently.
    * **Stateful & Resilient Ingester:** Features state management and retry logic.
    * **Transactional Inserts:** Guarantees atomic data writes on a per-block basis.
    * **V1 API Complete:** All key endpoints for querying blocks, transactions, and logs are implemented.
    * **Interactive Documentation:** The API is fully documented and testable via an integrated Swagger UI.

* **Next Steps (Focus on Performance & Enhancements):**
    1.  **Performance for Historical Sync:** Optimize bulk ingestion (e.g., explore batch JSON-RPC calls).
    2.  **Configuration:** Make ingester parameters (batch sizes, poll intervals) configurable via `.env`.
    3.  **Advanced API Filtering:** Enhance `POST /logs` with more complex filter logic (e.g., multiple addresses, OR logic for topics).
    4.  **Reorg Handling:** Implement logic in the ingester to gracefully handle chain reorganizations.

## 📜 License

This project is licensed under the [MIT License](LICENSE).
