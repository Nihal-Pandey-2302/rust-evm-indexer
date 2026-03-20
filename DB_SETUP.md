# 🗄️ Database Setup Guide — rust-evm-indexer

This guide walks you through installing PostgreSQL, creating the database and user, applying the schema, and preparing your environment.

---

## 📌 1. Install PostgreSQL (Ubuntu)

```bash
sudo apt update
sudo apt install postgresql postgresql-contrib
```

Verify PostgreSQL is running:
```bash
sudo systemctl status postgresql
```

---

## 📌 2. Create PostgreSQL User & Database

Enter PostgreSQL shell:
```bash
sudo -u postgres psql
```

Inside `psql`:
```sql
-- Create user
CREATE USER nihal WITH PASSWORD 'StrongP@ssw0rd!';

-- Create database
CREATE DATABASE evm_data_indexer OWNER nihal;

-- Grant permissions
GRANT ALL PRIVILEGES ON DATABASE evm_data_indexer TO nihal;
\q
```

---

## 📌 3. Apply Database Schema

The indexer uses a specific schema to handle block-level atomicity and chain reorganizations.

Connect to the database:
```bash
psql -h localhost -U nihal -d evm_data_indexer
```

Apply the following schema (from `init.sql`):

```sql
-- indexer status
CREATE TABLE IF NOT EXISTS indexer_status (
  indexer_name TEXT PRIMARY KEY,
  last_processed_block BIGINT,
  chain_head_at_last_poll BIGINT
);

-- blocks (PK is block_hash for reorg support)
CREATE TABLE IF NOT EXISTS blocks (
  block_hash TEXT PRIMARY KEY,
  block_number BIGINT NOT NULL,
  parent_hash TEXT NOT NULL,
  timestamp BIGINT NOT NULL,
  gas_used TEXT NOT NULL,
  gas_limit TEXT NOT NULL,
  base_fee_per_gas TEXT
);

CREATE INDEX IF NOT EXISTS idx_blocks_number ON blocks(block_number);

-- transactions
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

CREATE INDEX IF NOT EXISTS idx_transactions_block_number ON transactions(block_number);
CREATE INDEX IF NOT EXISTS idx_transactions_from_address ON transactions(from_address);
CREATE INDEX IF NOT EXISTS idx_transactions_to_address ON transactions(to_address);

-- logs
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

CREATE INDEX IF NOT EXISTS idx_logs_topic0_block ON logs(topic0, block_number);
CREATE INDEX IF NOT EXISTS idx_logs_composite ON logs(block_number, contract_address);
```

---

## 📌 4. Configuration

Create a `.env` file in the project root:

```env
ETH_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY
DATABASE_URL=postgres://nihal:StrongP%40ssw0rd%21@localhost:5432/evm_data_indexer
START_BLOCK=23900790
```

> **Note:** Password in the URL must be percent-encoded (e.g., `@` becomes `%40`).

---

## 📌 5. Run the Indexer

```bash
cargo run
```
