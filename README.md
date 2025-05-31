# EVM Indexer in Rust 🦀

A high-performance Ethereum Virtual Machine (EVM) data indexer and query API, built with Rust. This project features a **continuously running ingester** that fetches blocks, transactions, and event logs from an Ethereum node, storing them in PostgreSQL. A **concurrent REST API** provides queryable access to the indexed data. The V1 API with key lookup endpoints and robust log querying (including filtering and pagination) is implemented.

## 🌟 Project Goals & Motivation

* To gain practical experience in building robust systems with Rust.
* To deepen understanding of Ethereum's data structures, JSON-RPC API, and EVM internals.
* To explore efficient data ingestion (including continuous, stateful syncing), storage (PostgreSQL with `sqlx`), and API design patterns (with `axum`).
* To serve as a significant learning tool and portfolio piece for blockchain protocol engineering.

## ✨ Features

* **Data Ingestion:**
  * [x] Fetch historical blocks from an Ethereum node.
  * [x] Extract transactions from blocks.
  * [x] Extract event logs from transaction receipts.
  * **[x] Continuous polling for new blocks.**
  * **[x] State management to resume ingestion from the last successfully synced block (state stored in DB).**
  * **[x] Basic batch processing of blocks.**
  * **[x] Per-block data insertion within database transactions for atomicity.**
  * *(Full historical sync performance and advanced error handling/retries are future enhancements).*
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
  * **[x] Concurrent operation of ingester and API server using Tokio tasks.**
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

    It will prompt for the password. Once connected (you should see the `evm_data_indexer=>` prompt), execute the following SQL DDL statements to create the necessary tables (including the `indexer_status` table):

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
        CONSTRAINT fk_block_number_logs 
            FOREIGN KEY(block_number)
            REFERENCES blocks(block_number)
            ON DELETE CASCADE
    );

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
    -- Optional: You can insert an initial starting block for the ingester
    -- INSERT INTO indexer_status (indexer_name, last_processed_block) VALUES ('evm_main_sync', STARTING_BLOCK_NUMBER_HERE)
    -- ON CONFLICT (indexer_name) DO NOTHING;
    ```

    After executing these, you can type `\q` to exit `psql`.

5. **Build the project:**

    ```bash
    cargo build
    ```

6. **Run the Project (Concurrent Ingester & API Server):**

    ```bash
    cargo run
    ```

    This command will start both the continuous data ingester and the API server concurrently:
    * The **ingester** will run as a background Tokio task, automatically checking for new blocks based on its last synced state (or `DEFAULT_START_BLOCK` on first run from `main.rs`) and populating the database. You will see its logging output in the console (e.g., "INGESTER Cycle: Targeting blocks...").
    * The **API server** will start and listen on `http://127.0.0.1:3000` (by default), ready to serve requests using the data indexed. You will see its startup message (e.g., "API server listening on...").
    To stop both services, press `Ctrl+C` in the terminal.

## 🗺️ Project Status & Roadmap

* **Current Status:**
  * **Concurrent Operation:** Implemented concurrent execution of a continuous data ingester and the API server using `tokio::spawn`.
  * **Stateful Ingester:** Ingester now features state management (persisted in PostgreSQL's `indexer_status` table) for resumable and continuous syncing of recent blocks in batches.
  * **Transactional Inserts:** Database writes for each block's data (block, transactions, logs, and sync status) are performed within a single database transaction for improved atomicity.
  * **V1 API Complete:** REST API developed with `axum` provides key query capabilities:
    * Standardized JSON error handling (`ApiError`).
    * `POST /logs` with filtering by block range, specific `blockHash`, single contract `address`, and exact matches for `topic0`, `topic1`, `topic2`, `topic3`. Includes pagination (`page`, `pageSize`).
    * `GET /block/{identifier}` supporting lookup by block number or block hash.
    * `GET /transaction/{transaction_hash}` for transaction details.
  * **Code Organization:** Codebase well-organized into modules (`db.rs`, `api.rs`, `models.rs`, `api_models.rs`).

* **Next Steps (Focus on Ingester Robustness & Performance):**
    1. **Enhanced Ingester Error Handling & Retries:** Implement more specific retry logic with backoff for RPC calls (e.g., for `get_block_with_txs`, `get_transaction_receipt`) and transient database errors within the ingestion loop.
    2. **Performance for Historical Sync:** Address the N+1 query problem for transaction receipts more systematically for bulk ingestion. Explore options like batch JSON-RPC calls or a multi-pass ingestion strategy.
    3. **Configuration:** Make batch sizes, poll intervals, and default start blocks more easily configurable (e.g., via `.env` or command-line arguments).
    4. **(Further API Enhancements - V1.1 / V2):**
        * Advanced `getLogs` filtering (multiple addresses, OR logic for topics within a position, block tags like "latest").
        * New utility endpoints (e.g., transactions by address, with pagination).
        * Enhanced input validation for API parameters.
    5. **(Longer Term / Ongoing):**
        * Reorg handling for the ingester.
        * Full database type/index optimization (e.g., using `NUMERIC` for `U256` values, `TIMESTAMPTZ` for timestamps).
        * API Documentation (e.g., OpenAPI/Swagger).

## 📜 License

This project is licensed under the [MIT License](LICENSE).
