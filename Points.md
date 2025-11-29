1. Ingestion Loop (Async Fetching with Tokio)
Location: src/main.rs inside the run_continuous_ingester function.

How it works:

Async Task: The function is spawned as a background task using tokio::spawn (Line 270), allowing it to run concurrently with the API server.
Continuous Loop: It uses an infinite loop (Line 37) to continuously poll for new blocks.
Batching: It calculates a range of blocks to fetch (Lines 47-75) to process them in batches.
Concurrent Fetching: Inside the loop, it uses provider.get_block_with_txs (Line 100) and provider.get_transaction_receipt (Line 150) which are async calls awaited by the Tokio runtime.
2. Reorg Handling & Database Schema
Location: Schema is in DB_SETUP.md (and applied via SQL), logic would be in src/main.rs and src/db.rs.

Current Design:

Atomicity: The code uses Database Transactions (pool.begin().await at Line 96 of main.rs) to ensure that a block and all its transactions/logs are inserted together. If any part fails, the whole block is rolled back.
Reorg Handling Status: Full reorg handling is currently a "Next Step" (as noted in the README).
Currently, the blocks table has block_number as the Primary Key.
The insert query uses ON CONFLICT (block_number) DO NOTHING (Line 63 of db.rs).
Limitation: If a reorg occurs (the chain creates a new block at height X with a different hash), the current indexer will keep the old block because of DO NOTHING. It does not yet detect the hash mismatch and "rollback" or "replace" the old block.
To implement full reorg handling (Future Work): You would need to add logic in run_continuous_ingester to:

Fetch the latest synced block from the DB.
Compare its block_hash with the parent_hash of the new block from the RPC.
If they don't match, a reorg happened! You would then delete the last block(s) from the DB until you find a common ancestor.
