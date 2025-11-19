Great — here are your two ready-to-paste files:

* **`DB_SETUP.md`** (full PostgreSQL setup instructions)
* **Updated `README.md` section** with a clean link to the DB setup file

Everything is formatted in clean GitHub-compatible Markdown.

---

# 📄 **DB_SETUP.md (complete file — copy & paste into repo root)**

````md
# 🗄️ Database Setup Guide — rust-evm-indexer

This guide walks you through installing PostgreSQL, creating the database and user,
applying the schema, granting permissions, and preparing your environment to run the
Rust EVM indexer.

---

# 📌 1. Install PostgreSQL (Ubuntu)

```bash
sudo apt update
sudo apt install postgresql postgresql-contrib
````

Verify PostgreSQL is running:

```bash
sudo systemctl status postgresql
```

---

# 📌 2. Create PostgreSQL user (`nihal`) with password

Enter PostgreSQL shell:

```bash
sudo -u postgres psql
```

Inside `psql`:

```sql
CREATE USER nihal WITH PASSWORD 'StrongP@ssw0rd!';
```

Exit:

```sql
\q
```

---

# 📌 3. Create the database

```bash
sudo -u postgres createdb -O nihal evm_data_indexer
```

Verify:

```bash
sudo -u postgres psql -d evm_data_indexer -c "\l"
```

---

# 📌 4. Apply Database Schema

Connect to database:

```bash
sudo -u postgres psql -d evm_data_indexer
```

Paste the schema below:

```sql
-- indexer status
CREATE TABLE IF NOT EXISTS indexer_status (
  indexer_name TEXT PRIMARY KEY,
  last_processed_block BIGINT
);

-- blocks
CREATE TABLE IF NOT EXISTS blocks (
  block_number BIGINT PRIMARY KEY,
  block_hash TEXT NOT NULL,
  parent_hash TEXT NOT NULL,
  timestamp BIGINT NOT NULL,
  gas_used TEXT NOT NULL,
  gas_limit TEXT NOT NULL,
  base_fee_per_gas TEXT
);

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
```

Exit:

```sql
\q
```

---

# 📌 5. Fix Ownership & Permissions

If tables were created as `postgres`, fix them:

```bash
sudo -u postgres psql -d evm_data_indexer -c "ALTER TABLE indexer_status OWNER TO nihal;"
sudo -u postgres psql -d evm_data_indexer -c "ALTER TABLE blocks OWNER TO nihal;"
sudo -u postgres psql -d evm_data_indexer -c "ALTER TABLE transactions OWNER TO nihal;"
sudo -u postgres psql -d evm_data_indexer -c "ALTER TABLE logs OWNER TO nihal;"
```

Fix sequence owner:

```bash
SEQ=$(sudo -u postgres psql -d evm_data_indexer -Atc "SELECT pg_get_serial_sequence('logs','id');")
[ -n "$SEQ" ] && sudo -u postgres psql -d evm_data_indexer -c "ALTER SEQUENCE $SEQ OWNER TO nihal;"
```

Grant privileges:

```bash
sudo -u postgres psql -d evm_data_indexer -c "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO nihal;"
sudo -u postgres psql -d evm_data_indexer -c "GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO nihal;"
```

Default privileges for future tables:

```bash
sudo -u postgres psql -d evm_data_indexer -c "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO nihal;"
sudo -u postgres psql -d evm_data_indexer -c "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO nihal;"
```

---

# 📌 6. Create `.env` file

Create `.env` in project root:

```
ETH_RPC_URL=https://mainnet.infura.io/v3/<YOUR_INFURA_KEY>
DATABASE_URL=postgres://nihal:StrongP%40ssw0rd%21@localhost/evm_data_indexer
```

> **Note:** Password must be URL-encoded
> `@ → %40`
> `! → %21`

---

# 📌 7. Ensure Rust loads `.env`

Add to `main.rs`:

```rust
dotenv::dotenv().ok();
```

Add to `Cargo.toml`:

```toml
dotenv = "0.15"
```

---

# 📌 8. Run the indexer

```bash
cargo run
```

Expected output:

```
Successfully connected to Ethereum provider.
Successfully connected to database.
API server listening...
Ingester running...
```

---

# 🎉 Done

Your PostgreSQL database is now ready for the EVM indexer.



