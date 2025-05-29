# EVM Indexer in Rust 🦀

A high-performance Ethereum Virtual Machine (EVM) historical data ingester and query API, built with Rust. This project focuses on ingesting blocks, transactions, and event logs from an Ethereum node, storing them efficiently in PostgreSQL, and providing a queryable REST API. Core ingestion and storage for recent blocks is functional, and a foundational API with key endpoints is in place.

## 🌟 Project Goals & Motivation

* To gain practical experience in building robust systems with Rust.
* To deepen understanding of Ethereum's data structures, JSON-RPC API, and EVM internals.
* To explore efficient data ingestion, storage (PostgreSQL with `sqlx`), and API design patterns (with `axum`).
* To serve as a significant learning tool and portfolio piece for blockchain protocol engineering.

## ✨ Features

* **Data Ingestion:**
  * [x] Fetch historical blocks from an Ethereum node.
  * [x] Extract transactions from blocks.
  * [x] Extract event logs from transaction receipts.
  * *(Current implementation processes recent blocks; continuous/full historical sync is a future enhancement).*
* **Storage:**
  * [x] Store ingested data (blocks, transactions, logs) in a PostgreSQL database.
  * [x] Designed and implemented v1 database schema; further refinement planned.
* **API (using Axum):**
  * [x] Basic REST API server setup.
  * [x] `POST /logs` endpoint with basic filtering (block range, single address, topic0) using `sqlx::QueryBuilder`.
  * [x] `GET /block/{block_number}` endpoint to fetch block data.
  * [x] `GET /transaction/{transaction_hash}` endpoint to fetch transaction data.
  * [ ] Advanced filtering for `/logs` (multiple addresses/topics, `blockHash` filter, block tags) pending.
  * [ ] Standardized JSON error handling pending.
  * [ ] Pagination for list responses pending.
* **Core:**
  * Built with Rust for performance and safety.
  * [x] Asynchronous processing using Tokio.
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
    You'll need a dedicated database and user for the indexer. You can create these using `psql` (PostgreSQL's command-line interface).

    First, log in to `psql` as a superuser (e.g., `postgres`):

    ```bash
    sudo -u postgres psql
    ```

    Then, execute the following SQL commands. Replace `YOUR_CHOSEN_PASSWORD` with a strong password for the new user.

    ```sql
    -- Create a dedicated user for the indexer
    CREATE USER indexer_user WITH PASSWORD 'YOUR_CHOSEN_PASSWORD';

    -- Create the database and set the new user as the owner
    CREATE DATABASE evm_data_indexer OWNER indexer_user;

    -- Exit psql for now
    \q
    ```

3. **Set up your environment variables:**
    Create a file named `.env` in the root of the project directory. Add your Ethereum RPC URL and your PostgreSQL database connection URL, using the credentials you just created:

    ```env
    # .env
    ETH_RPC_URL=YOUR_ETHEREUM_NODE_RPC_URL_HERE
    DATABASE_URL=postgres://indexer_user:YOUR_CHOSEN_PASSWORD@localhost:5432/evm_data_indexer
    ```

    * Replace `YOUR_ETHEREUM_NODE_RPC_URL_HERE` with your actual Ethereum RPC URL.
    * Replace `YOUR_CHOSEN_PASSWORD` with the password you set for `indexer_user`.
    * Adjust `localhost:5432` if your PostgreSQL server runs on a different host or port.
    * Ensure the database name (`evm_data_indexer`) matches what you created.
    **Note:** The `.env` file is included in `.gitignore` and should not be committed to the repository.

4. **Create Database Tables:**
    Connect to your newly created database as the `indexer_user`:

    ```bash
    psql -U indexer_user -d evm_data_indexer -h localhost
    ```

    It will prompt for the password. Once connected (you should see the `evm_data_indexer=>` prompt), execute the following SQL DDL statements to create the necessary tables:

    ```sql
    -- Create the 'blocks' table
    CREATE TABLE blocks (
        block_number BIGINT PRIMARY KEY,
        block_hash VARCHAR(66) UNIQUE NOT NULL,
        parent_hash VARCHAR(66) NOT NULL,
        timestamp BIGINT NOT NULL, -- Unix timestamp
        gas_used TEXT NOT NULL,    -- Storing U256 as string
        gas_limit TEXT NOT NULL,   -- Storing U256 as string
        base_fee_per_gas TEXT      -- Storing Option<U256> as string
    );

    -- Create the 'transactions' table
    CREATE TABLE transactions (
        tx_hash VARCHAR(66) PRIMARY KEY,
        block_number BIGINT NOT NULL,
        block_hash VARCHAR(66) NOT NULL,
        transaction_index BIGINT,
        from_address VARCHAR(42) NOT NULL,
        to_address VARCHAR(42),
        value TEXT NOT NULL, -- Storing U256 as string
        gas_price TEXT,      -- Storing Option<U256> as string
        max_fee_per_gas TEXT, -- Storing Option<U256> as string
        max_priority_fee_per_gas TEXT, -- Storing Option<U256> as string
        gas_provided TEXT NOT NULL, -- Gas limit for the tx, U256 as string
        input_data TEXT,
        status SMALLINT,     -- 0 for failure, 1 for success
        CONSTRAINT fk_block_number_transactions
            FOREIGN KEY(block_number)
            REFERENCES blocks(block_number)
            ON DELETE CASCADE
    );

    -- Create indexes for 'transactions' table
    CREATE INDEX IF NOT EXISTS idx_transactions_block_number ON transactions(block_number);
    CREATE INDEX IF NOT EXISTS idx_transactions_from_address ON transactions(from_address);
    CREATE INDEX IF NOT EXISTS idx_transactions_to_address ON transactions(to_address);

    -- Create the 'logs' table
    CREATE TABLE logs (
        id BIGSERIAL PRIMARY KEY, -- Auto-incrementing primary key
        log_index_in_tx BIGINT,
        transaction_hash VARCHAR(66) NOT NULL,
        transaction_index_in_block BIGINT,
        block_number BIGINT NOT NULL,
        block_hash VARCHAR(66) NOT NULL,
        contract_address VARCHAR(42) NOT NULL,
        data TEXT,
        topic0 VARCHAR(66),
        topic1 VARCHAR(66),
        topic2 VARCHAR(66),
        topic3 VARCHAR(66),
        all_topics TEXT[] NOT NULL, -- Stores all topics as an array of text
        CONSTRAINT fk_transaction_hash_logs
            FOREIGN KEY(transaction_hash)
            REFERENCES transactions(tx_hash)
            ON DELETE CASCADE,
        CONSTRAINT fk_block_number_logs -- Optional: Can also link directly to blocks
            FOREIGN KEY(block_number)
            REFERENCES blocks(block_number)
            ON DELETE CASCADE
    );

    -- Create indexes for 'logs' table
    CREATE INDEX IF NOT EXISTS idx_logs_transaction_hash ON logs(transaction_hash);
    CREATE INDEX IF NOT EXISTS idx_logs_contract_address ON logs(contract_address);
    CREATE INDEX IF NOT EXISTS idx_logs_topic0 ON logs(topic0);
    CREATE INDEX IF NOT EXISTS idx_logs_all_topics_gin ON logs USING GIN (all_topics);
    ```

    After executing these, you can type `\q` to exit `psql`.

5. **Build the project:**

    ```bash
    cargo build
    ```

6. **Run the API Server:**

    ```bash
    cargo run
    ```

    This will start the API server (listening on `http://127.0.0.1:3000` by default). The data ingestion logic in `main.rs` is currently commented out to focus on API development. To populate the database initially:
    * You'll need to temporarily uncomment the ingestion loop in `src/main.rs` (the part that fetches blocks and calls the `db::insert_...` functions).
    * Run `cargo run` to ingest some data.
    * Then, comment out the ingestion loop again to run only the API server.
    *(Future enhancements will involve making ingestion a separate command or background task.)*

## 🗺️ Project Status & Roadmap

* **Current Status:**
  * Core data ingestion pipeline implemented: successfully fetches and stores blocks, transactions, and event logs into a PostgreSQL database for recent blocks.
  * REST API developed with `axum`, providing initial query capabilities:
    * `POST /logs` with basic filtering (block range, single address, topic0) using `sqlx::QueryBuilder`.
    * `GET /block/{block_number}`.
    * `GET /transaction/{transaction_hash}`.
  * Codebase organized into modules for database interactions (`db.rs`), API handling (`api.rs`), and data models (`models.rs`, `api_models.rs`).

* **Next Steps (Focus on API Enhancement):**
    1. Implement standardized JSON error handling for the API.
    2. Enhance `/logs` endpoint:
        * Add filtering for `topic1`, `topic2`, `topic3`.
        * Implement `blockHash` filter.
    3. Enhance `/block/{identifier}` to accept block hash in addition to block number.
    4. (Further Out)
        * Implement pagination for API endpoints returning lists.
        * Advanced topic filtering for `/logs` (arrays of topics, OR logic).
        * Develop ingester for continuous operation (state management, robust error handling).
        * Performance optimizations for historical data sync.

## 📜 License

This project is licensed under the [MIT License](LICENSE).
