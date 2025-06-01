# EVM Indexer in Rust 🦀

A high-performance Ethereum Virtual Machine (EVM) data indexer and query API, built with Rust. This project features a **continuously running ingester** that fetches blocks, transactions, and event logs from an Ethereum node, storing them in PostgreSQL. A **concurrent REST API** provides queryable access to the indexed data. The V1 API with key lookup endpoints and robust log querying (including filtering and pagination) is implemented, and the ingester includes retry mechanisms for core RPC calls.

## 🌟 Project Goals & Motivation

* To gain practical experience in building robust systems with Rust.
* To deepen understanding of Ethereum's data structures, JSON-RPC API, and EVM internals.
* To explore efficient data ingestion (including continuous, stateful, and resilient syncing), storage (PostgreSQL with `sqlx`), and API design patterns (with `axum`).
* To serve as a significant learning tool and portfolio piece for blockchain protocol engineering.

## ✨ Features

* **Data Ingestion:**
  * [x] Fetch historical blocks from an Ethereum node.
  * [x] Extract transactions from blocks.
  * [x] Extract event logs from transaction receipts.
  * [x] Continuous polling for new blocks.
  * [x] State management to resume ingestion from the last successfully synced block (state stored in DB).
  * [x] Basic batch processing of blocks.
  * [x] Per-block data insertion within database transactions for atomicity.
  * **[x] Retry logic with exponential backoff implemented for critical RPC calls (`get_block_with_txs`, `get_transaction_receipt`).**
  * *(Full historical sync performance and more comprehensive ingester error handling are future enhancements).*
* **Storage:**
  * [x] Store ingested data (blocks, transactions, logs) in a PostgreSQL database.
  * [x] Designed and implemented v1 database schema; further refinement planned.
* **API (using Axum):**
  * [x] Basic REST API server setup.
  * [x] Standardized JSON error handling implemented (`ApiError`).
  * [x] `POST /logs` endpoint with:
    * Filtering by block range (`fromBlock`, `toBlock`).
    * Filtering by specific `blockHash` (overrides block range).
    * Filtering by single contract `address`.
    * Filtering by `topic0`, `topic1`, `topic2`, and `topic3` (exact match).
    * Pagination (`page`, `pageSize`).
  * [x] `GET /block/{identifier}` endpoint (accepts block number or hash).
  * [x] `GET /transaction/{transaction_hash}` endpoint.
  * [ ] Advanced filtering for `/logs` (multiple addresses, OR logic for topics, block tags) pending.
* **Core:**
  * Built with Rust for performance and safety.
  * [x] Asynchronous processing using Tokio.
  * [x] Concurrent operation of ingester and API server using Tokio tasks.
  * [x] Interaction with Ethereum nodes via `ethers-rs`.
  * [x] Database interaction using `sqlx` with PostgreSQL.
  * [x] Modular code structure (models, database logic, API handlers).

## 🛠️ Tech Stack

* **Language:** Rust (Edition 2021)
* **Async Runtime:** Tokio
* **Ethereum Interaction:** `ethers-rs`
* **Database:** PostgreSQL (using `sqlx`)
* **API Framework:** Axum
* **Configuration:** `dotenvy` (for environment variables)
* **Serialization:** `serde` (for JSON request/responses)

## 🚀 Getting Started

### Prerequisites

* Rust toolchain (visit [rustup.rs](https://rustup.rs/))
* PostgreSQL server installed and running.
* Access to an Ethereum JSON-RPC endpoint (e.g., from Infura, Alchemy, or a local node).

### Installation & Running

1. **Clone the repository:**

    ```bash
    git clone [https://github.com/Nihal-Pandey-2302/rust-evm-indexer.git](https://github.com/Nihal-Pandey-2302/rust-evm-indexer.git)
    cd rust-evm-indexer
    ```

2. **Set up PostgreSQL Database & User:**
    (Instructions as you currently have them - create `indexer_user` and `evm_data_indexer` database)

    ```bash
    sudo -u postgres psql
    ```

    ```sql
    CREATE USER indexer_user WITH PASSWORD 'YOUR_CHOSEN_PASSWORD';
    CREATE DATABASE evm_data_indexer OWNER indexer_user;
    \q
    ```

3. **Set up your environment variables:**
    Create `.env` file:

    ```env
    # .env
    ETH_RPC_URL=YOUR_ETHEREUM_NODE_RPC_URL_HERE
    DATABASE_URL=postgres://indexer_user:YOUR_CHOSEN_PASSWORD@localhost:5432/evm_data_indexer
    ```

    (Instructions as you currently have them)

4. **Create Database Tables:**
    Connect to your database:

    ```bash
    psql -U indexer_user -d evm_data_indexer -h localhost
    ```

    Execute DDL statements:

    ```sql
    -- Create the 'blocks' table
    CREATE TABLE blocks ( /* ... as you have it ... */ );

    -- Create the 'transactions' table
    CREATE TABLE transactions ( /* ... as you have it ... */ );
    -- Create indexes for 'transactions' table
    CREATE INDEX IF NOT EXISTS idx_transactions_block_number ON transactions(block_number);
    CREATE INDEX IF NOT EXISTS idx_transactions_from_address ON transactions(from_address);
    CREATE INDEX IF NOT EXISTS idx_transactions_to_address ON transactions(to_address);

    -- Create the 'logs' table
    CREATE TABLE logs ( /* ... as you have it ... */ );
    -- Create indexes for 'logs' table
    CREATE INDEX IF NOT EXISTS idx_logs_transaction_hash ON logs(transaction_hash);
    CREATE INDEX IF NOT EXISTS idx_logs_contract_address ON logs(contract_address);
    CREATE INDEX IF NOT EXISTS idx_logs_topic0 ON logs(topic0);
    CREATE INDEX IF NOT EXISTS idx_logs_topic1 ON logs(topic1);
    CREATE INDEX IF NOT EXISTS idx_logs_topic2 ON logs(topic2);
    CREATE INDEX IF NOT EXISTS idx_logs_topic3 ON logs(topic3);
    CREATE INDEX IF NOT EXISTS idx_logs_all_topics_gin ON logs USING GIN (all_topics);

    -- Create the 'indexer_status' table for state management
    CREATE TABLE indexer_status (
        indexer_name VARCHAR(100) PRIMARY KEY,
        last_processed_block BIGINT NOT NULL
    );
    ```

    Then `\q` to exit `psql`.

5. **Build the project:**

    ```bash
    cargo build
    ```

6. **Run the Project (Concurrent Ingester & API Server):**

    ```bash
    cargo run
    ```

    This command will start both the continuous data ingester and the API server concurrently. The ingester runs as a background Tokio task, automatically syncing new blocks. The API server listens on `http://127.0.0.1:3000`. To stop both, press `Ctrl+C`.

## 🗺️ Project Status & Roadmap

* **Current Status:**
  * **Concurrent Operation:** Implemented concurrent execution of a continuous data ingester and the API server.
  * **Stateful & Resilient Ingester:** Ingester features state management (persisted in PostgreSQL) for resumable syncing and **retry logic with backoff for key RPC calls** (`get_block_with_txs`, `get_transaction_receipt`).
  * **Transactional Inserts:** Per-block data insertion (block, transactions, logs, sync status) occurs within database transactions.
  * **V1 API Complete:** REST API with `axum` provides standardized JSON error handling, `POST /logs` (with filtering by block range/hash, address, topics 0-3, and pagination), `GET /block/{identifier}` (number/hash), and `GET /transaction/{hash}`.
  * **Code Organization:** Well-organized into modules (`db.rs`, `api.rs`, `models.rs`, `api_models.rs`).

* **Next Steps (Focus on Ingester Performance & Further Enhancements):**
    1. **Performance for Historical Sync:** Address the N+1 query problem for transaction receipts more systematically for bulk ingestion (e.g., explore batch JSON-RPC calls).
    2. **Configuration:** Make ingester parameters like batch sizes, poll intervals, and default start blocks more easily configurable (e.g., via `.env` or command-line arguments).
    3. **More Comprehensive Ingester Error Handling:** Further refine error handling for different classes of RPC or database errors during ingestion.
    4. **(Further API Enhancements - V1.1 / V2):**
        * Advanced `getLogs` filtering (multiple addresses, OR logic for topics, block tags).
        * New utility endpoints (e.g., transactions by address, with pagination).
    5. **(Longer Term / Ongoing):**
        * Reorg handling for the ingester.
        * API Documentation (e.g., OpenAPI/Swagger).
        * Database schema/type refinements.

## 📜 License

This project is licensed under the [MIT License](LICENSE).
