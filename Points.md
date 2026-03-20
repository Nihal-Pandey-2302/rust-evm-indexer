1. Ingestion Loop (Async Fetching with Tokio)
Location: `src/main.rs` inside the `run_continuous_ingester` function.

How it works:
- **Async Task**: Spawned as a background task via `tokio::spawn`, enabling concurrent operation with the API server.
- **Continuous Loop**: Uses an infinite loop with a 10-second poll interval to monitor the chain head.
- **Dynamic Batching**: Automatically calculates the block range to fetch, processing up to 5 blocks per batch to balance throughput and latency.
- **Parallel Receipt Fetching**: Utilizes `futures::stream::buffer_unordered(10)` to fetch transaction receipts concurrently, significantly reducing I/O wait times compared to sequential fetching.

2. Reorg Handling & Database Architecture
Location: Logic in `src/main.rs` and `src/db.rs`; schema in `init.sql`.

Current Design:
- **Canonical Chain Awareness**: The `blocks` table now uses `block_hash` as the Primary Key. This allows the indexer to store multiple blocks at the same height (e.g., in a fork scenario) without constraint violations.
- **Reorg Detection**: On each ingestion cycle, the `parent_hash` of the new block is compared against the stored `block_hash` of the preceding block in the database.
- **DELETE-based Rollback**: If a reorg is detected, the system executes `rollback_from_height()`, which deletes all logs, transactions, and blocks from the fork height onwards. This ensures the database state remains strictly canonical.
- **Atomicity**: Every block is processed within a single PostgreSQL transaction (`pool.begin()`). The block record, all transactions, and all logs are committed or rolled back as a single atomic unit.

3. Performance & Pagination
Location: `src/api.rs` and `init.sql`.

- **Composite Cursor Pagination**: The `/logs` endpoint implements cursor-based pagination using `(block_number, id)`. This ensures stable, deterministic ordering and O(log N) query performance by leveraging a B-tree index, avoiding the pitfalls of `OFFSET` at scale.
- **Optimized Indexing**: Includes a high-value composite index on `(topic0, block_number)` for fast log filtering by event type and time range.
- **Ingestion Lag Monitoring**: The `/stats` endpoint computes real-time synchronization lag by comparing the current RPC chain head with the last indexed block.
