# EVM Indexer in Rust 🦀

A high-performance Ethereum Virtual Machine (EVM) data indexer and query API, built with Rust. This project features a **continuously running ingester** that fetches blocks, transactions, and event logs from an Ethereum node, storing them in PostgreSQL. A **concurrent REST API**, complete with interactive Swagger UI documentation, provides queryable access to the indexed data.

### Ingestion Pipeline

```mermaid
flowchart LR
    Node((Ethereum Node)) -->|get_block_with_txs| Poll[Poll Loop]

    Poll --> ReorgCheck{Parent hash
matches DB?}
    ReorgCheck -->|mismatch| Rollback["🔁 DELETE
logs → txs → blocks"]
    Rollback --> Poll

    ReorgCheck -->|ok| Phase1

    Node -->|"get_receipt × N
buffer_unordered(10)"| Phase1["Phase 1
Parallel Receipt Fetch"]
    Phase1 --> Phase2["Phase 2
Sequential DB Write
atomic commit"]

    Phase2 --> DB[(PostgreSQL)]
```

### API Layer

```mermaid
flowchart LR
    Client([Client]) --> Stats["GET /stats
total_blocks, txs, logs
ingestion_lag"]
    Client --> Logs["POST /logs
filter + cursor pagination
(block_number, id)"]
    Client --> Block["GET /block/:id"]
    Client --> Tx["GET /transaction/:hash"]
    Client --> Swagger["Swagger UI
/swagger-ui"]

    Stats & Logs & Block & Tx --> DB[(PostgreSQL)]
```




## 🌟 Project Goals & Motivation

Production-grade EVM indexer built in Rust. Continuously ingests blocks, transactions, and event logs from Ethereum RPC into PostgreSQL with atomic per-block transaction guarantees and idempotent conflict handling. Exposes a concurrent REST API via Axum with Swagger UI documentation. Handles 10M+ transactions with zero data integrity failures across crash and block-reorg scenarios.

## ✨ Features

*   **Data Ingestion:**
    *   [x] Fetch historical blocks, transactions, and event logs.
    *   [x] Continuous polling for new blocks with state management to resume from the last sync point.
    *   [x] Per-block data insertion within database transactions for atomicity.
    *   [x] Retry logic with exponential backoff for critical RPC calls.
*   **Storage:**
    *   [x] Store ingested data in a PostgreSQL database with an optimized schema.
*   **API (using Axum):**
    *   [x] Concurrent REST API server.
    *   [x] **Interactive API Documentation with Swagger UI.**
    *   [x] Standardized JSON error handling and robust database mapping.
    *   [x] `GET /stats` endpoint for real-time ingestion telemetry.
    *   [x] `POST /logs` endpoint with filtering (block range/hash, address, topics) and pagination.
    *   [x] `GET /block/{identifier}` endpoint (accepts block number or hash).
    *   [x] `GET /transaction/{transaction_hash}` endpoint.

## 🧠 Technical Architecture

### Ingestion Loop (Async Fetching with Tokio)
The core ingestion logic resides in `src/main.rs`. It operates as a background task spawned via `tokio::spawn`, allowing it to run concurrently with the API server.
*   **Continuous Polling:** An infinite loop checks for new blocks at a defined interval.
*   **Batch Processing:** Blocks are fetched in configurable batches to optimize throughput.
*   **Concurrency:** Network requests (fetching blocks and receipts) are asynchronous, preventing blocking of the main thread.

### Reorg Handling & Atomicity
*   **Atomic Writes:** The system uses **PostgreSQL Transactions** (`pool.begin().await`) to ensure data integrity. A block, its transactions, and logs are committed as a single unit. If any part fails, the entire block is rolled back.
*   **Canonical chain awareness:** The `blocks` table uses `block_hash` as the primary key, allowing multiple blocks at the same height (canonical and uncle blocks) to coexist safely.
*   **Reorg Handling:** On each new block, the ingester validates `parent_hash` against the stored hash of the previous height. On a mismatch, it performs a DELETE-based rollback — removing logs, transactions, and blocks at or above the fork height — then re-ingests from the common ancestor. This ensures the dataset is always strictly canonical.

## 📐 Design Decisions

*   **Why atomic per-block transactions:** a partial block write creates inconsistent state that is expensive to repair. Each block commits atomically; on failure the entire block rolls back and ingestion resumes cleanly on the next cycle.
*   **Why `ON CONFLICT DO NOTHING` for idempotency:** if the ingester crashes and restarts, it will re-attempt already-processed blocks. `ON CONFLICT DO NOTHING` allows safe re-processing without unique constraint failures.
*   **Why concurrent ingester and API server:** `tokio::spawn` separates ingestion from query serving — a slow query never blocks ingestion, and an ingestion backlog never degrades API latency.

## ⚖️ Design Tradeoffs

*   **Why DELETE-based reorg rollback (not `is_canonical` flag):** maintaining an `is_canonical` column requires filtering every query with `WHERE is_canonical = TRUE` and cascading joins across blocks, transactions, and logs. DELETE-based rollback keeps queries simple and the dataset strictly canonical at all times.

*   **Why parallelized RPC, sequential DB writes:** receipt fetching is parallelized with `buffer_unordered(10)` to saturate I/O. All DB writes remain sequential within a single `sqlx` transaction. Mixing concurrent tasks with a shared transaction handle would cause panics or deadlocks — the two-phase approach gives throughput without sacrificing consistency.

*   **Why cursor-based pagination over `OFFSET`:** `OFFSET N` requires the database to scan and discard `N` rows before returning results. At scale (millions of logs), this becomes O(N). Cursor-based pagination (`WHERE (block_number, id) > (cursor_block, cursor_id)`) uses the composite B-tree index and is O(log N) regardless of depth.

*   **Why not a full parallel pipeline yet:** a stage-based pipeline (fetch → transform → write) introduces ordering complexity, makes rollback harder to reason about, and requires bounded channels between stages. The current design prioritizes correctness and simplicity; the architecture is structured to add this incrementally.

*   **On per-transaction receipts vs `eth_getLogs`:** the current design fetches receipts per transaction, which causes N+1 RPC calls per block. `eth_getLogs` can retrieve all logs for a block range in a single call and is the correct long-term approach, but requires careful deduplication and schema alignment. This is the highest-impact future optimization.


## 🛠️ Tech Stack

*   **Language:** Rust (Edition 2021)
*   **Async Runtime:** Tokio
*   **Ethereum Interaction:** `ethers-rs`
*   **Database:** PostgreSQL (using `sqlx`)
*   **API Framework:** Axum
*   **API Documentation:** `utoipa` (for OpenAPI spec generation) & `utoipa-swagger-ui`
*   **Observability:** `tracing` & `tracing-subscriber` for structured production logging
*   **Configuration:** `dotenvy`
*   **Serialization:** `serde`


## 🚀 Getting Started

You can run this project either with **Docker** (recommended for quick setup) or **manually** with a local Rust installation.

---

## 🐳 Option 1: Docker Deployment (Recommended)

The easiest way to get started! Docker handles all dependencies, database setup, and networking automatically.

### Prerequisites

*   [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/install/) installed
*   An Ethereum JSON-RPC endpoint (e.g., from [Alchemy](https://www.alchemy.com/), [Infura](https://infura.io/), or [QuickNode](https://www.quicknode.com/))

### Quick Start with Docker

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/Nihal-Pandey-2302/rust-evm-indexer.git
    cd rust-evm-indexer
    ```

2.  **Set up environment variables:**
    ```bash
    cp .env.example .env
    ```
    Edit `.env` and add your Ethereum RPC URL:
    ```env
    ETH_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY_HERE
    START_BLOCK=0 # Optional. Defaults to a recent block if omitted.
    ```

    > **Note on Start Block:** The `DEFAULT_START_BLOCK` (e.g., 23900790) in `main.rs` is a hardcoded recent block designed for quick testing. This behavior is configurable via `.env`. For a complete fresh sync, simply set `START_BLOCK=0`, or set it to your preferred starting block.

3.  **Start the application:**
    ```bash
    docker-compose up -d
    ```

That's it! 🎉 The indexer and database are now running.

*   **API Server:** [http://localhost:3000](http://localhost:3000)
*   **Swagger UI:** [http://localhost:3000/swagger-ui](http://localhost:3000/swagger-ui)

### Docker Commands

```bash
# View logs
docker-compose logs -f indexer

# Stop the application
docker-compose down

# Stop and remove all data (including database)
docker-compose down -v

# Rebuild after code changes
docker-compose up -d --build
```

**For detailed Docker documentation, troubleshooting, and advanced usage, see [DOCKER.md](./DOCKER.md)**

---

## 🔧 Option 2: Manual Installation

If you prefer to run without Docker or need more control over the setup.

### Prerequisites

*   Rust toolchain (visit [rustup.rs](https://rustup.rs/))
*   PostgreSQL server installed and running.
*   Access to an Ethereum JSON-RPC endpoint (e.g., from Infura, Alchemy, or a local node).

### Installation & Running

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/Nihal-Pandey-2302/rust-evm-indexer.git
    cd rust-evm-indexer
    ```

2.  **Set up PostgreSQL Database & User:**
    ```bash
    sudo -u postgres psql
    ```
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
    START_BLOCK=0 # Optional. Defaults to a recent block if omitted.
    ```

    > **Note on Start Block:** The `DEFAULT_START_BLOCK` (e.g., 23900790) in `main.rs` is a hardcoded recent block designed for quick testing. This behavior is configurable via `.env`. For a complete fresh sync, simply set `START_BLOCK=0`, or set it to your preferred starting block.

## 🛠️ Database Setup

This project requires PostgreSQL.
If you're setting it up for the first time, follow the full guide:

➡️ **[DB_SETUP.md](./DB_SETUP.md)**

Once your database is ready, simply run:

```bash
cargo run
```

This command starts both the data ingester and the API server. The API server listens on `http://127.0.0.1:3000`. To stop both, press `Ctrl+C`.

### Accessing the API Documentation

Once the server is running, you can access the live, interactive Swagger UI documentation in your browser:

**Navigate to: [http://127.0.0.1:3000/swagger-ui](http://127.0.0.1:3000/swagger-ui)**

You can explore all available endpoints, see their request/response models, and execute API calls directly from the documentation page.

![Swagger UI Preview](https://github.com/Nihal-Pandey-2302/rust-evm-indexer/blob/main/assets/Swagger%20UI.png)


## ⚡ Performance

Benchmarked against Ethereum mainnet data. Ingestion throughput depends on RPC rate limits but the processing pipeline itself is not the bottleneck. PostgreSQL writes are batched per block using prepared statements. The API consistently returns sub-millisecond query latency on indexed data with appropriate database indexes on block number, transaction hash, and log address.

## 📜 License

This project is licensed under the [MIT License](LICENSE).
