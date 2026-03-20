-- Database initialization script for EVM Indexer
-- This script creates all necessary tables and indexes

-- indexer status table
CREATE TABLE IF NOT EXISTS indexer_status (
  indexer_name TEXT PRIMARY KEY,
  last_processed_block BIGINT,
  chain_head_at_last_poll BIGINT
);

-- blocks table
-- PK is block_hash to allow multiple blocks at the same height (canonical + uncle blocks)
CREATE TABLE IF NOT EXISTS blocks (
  block_hash TEXT PRIMARY KEY,
  block_number BIGINT NOT NULL,
  parent_hash TEXT NOT NULL,
  timestamp BIGINT NOT NULL,
  gas_used TEXT NOT NULL,
  gas_limit TEXT NOT NULL,
  base_fee_per_gas TEXT
);

-- Index on block_number for fast height lookups (non-unique: multiple blocks can share a height)
CREATE INDEX IF NOT EXISTS idx_blocks_number ON blocks(block_number);

-- transactions table
CREATE TABLE IF NOT EXISTS transactions (
  tx_hash TEXT PRIMARY KEY,
  block_number BIGINT NOT NULL,
  block_hash TEXT NOT NULL,
  transaction_index BIGINT,
  from_address TEXT NOT NULL,
  to_address TEXT,
  value TEXT NOT NULL,
  gas_price TEXT,
  max_fee_per_gas TEXT,
  max_priority_fee_per_gas TEXT,
  gas_provided TEXT NOT NULL,
  input_data BYTEA,
  status SMALLINT
);

-- Create indexes for common transaction queries
CREATE INDEX IF NOT EXISTS idx_transactions_block_number ON transactions(block_number);
CREATE INDEX IF NOT EXISTS idx_transactions_from_address ON transactions(from_address);
CREATE INDEX IF NOT EXISTS idx_transactions_to_address ON transactions(to_address);

-- logs table
CREATE TABLE IF NOT EXISTS logs (
  id BIGSERIAL PRIMARY KEY,
  log_index_in_tx BIGINT,
  transaction_hash TEXT NOT NULL,
  transaction_index_in_block BIGINT,
  block_number BIGINT NOT NULL,
  block_hash TEXT NOT NULL,
  contract_address TEXT NOT NULL,
  data BYTEA,
  topic0 TEXT,
  topic1 TEXT,
  topic2 TEXT,
  topic3 TEXT,
  all_topics TEXT[]
);

-- Create indexes for common log queries
CREATE INDEX IF NOT EXISTS idx_logs_block_number ON logs(block_number);
CREATE INDEX IF NOT EXISTS idx_logs_contract_address ON logs(contract_address);
CREATE INDEX IF NOT EXISTS idx_logs_transaction_hash ON logs(transaction_hash);
CREATE INDEX IF NOT EXISTS idx_logs_topic0 ON logs(topic0);
CREATE INDEX IF NOT EXISTS idx_logs_topic1 ON logs(topic1);
-- High-selectivity composite: most log queries filter by event signature + block range
CREATE INDEX IF NOT EXISTS idx_logs_topic0_block ON logs(topic0, block_number);
-- Composite for address + block range queries
CREATE INDEX IF NOT EXISTS idx_logs_composite ON logs(block_number, contract_address);
