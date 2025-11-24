# 🐳 Docker Deployment Guide

This guide provides detailed information about running the EVM Indexer using Docker.

## 📋 Table of Contents

- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Docker Commands](#docker-commands)
- [Troubleshooting](#troubleshooting)
- [Advanced Usage](#advanced-usage)

---

## 🏗️ Architecture

The Docker setup consists of two services:

1. **PostgreSQL Database** (`postgres`)
   - Image: `postgres:16-alpine`
   - Port: `5432`
   - Persistent storage via Docker volume
   - Auto-initialization with schema from `init.sql`

2. **EVM Indexer Application** (`indexer`)
   - Built from source using multi-stage Dockerfile
   - Port: `3000` (API server)
   - Depends on PostgreSQL with health checks
   - Automatically connects to database

Both services run on an isolated Docker network for secure communication.

---

## 🚀 Quick Start

### Prerequisites

- Docker Engine 20.10+ ([Install Docker](https://docs.docker.com/get-docker/))
- Docker Compose 2.0+ ([Install Compose](https://docs.docker.com/compose/install/))
- Ethereum RPC endpoint (Alchemy, Infura, or QuickNode)

### Steps

1. **Clone and navigate to the project:**
   ```bash
   git clone https://github.com/Nihal-Pandey-2302/rust-evm-indexer.git
   cd rust-evm-indexer
   ```

2. **Configure environment:**
   ```bash
   cp .env.example .env
   nano .env  # or use your preferred editor
   ```
   
   Update `ETH_RPC_URL` with your actual RPC endpoint:
   ```env
   ETH_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY
   ```

3. **Start services:**
   ```bash
   docker-compose up -d
   ```

4. **Verify deployment:**
   ```bash
   docker-compose ps
   docker-compose logs -f indexer
   ```

5. **Access the API:**
   - API: http://localhost:3000
   - Swagger UI: http://localhost:3000/swagger-ui

---

## ⚙️ Configuration

### Environment Variables

The following environment variables can be configured in `.env`:

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `ETH_RPC_URL` | Ethereum JSON-RPC endpoint | - | ✅ Yes |
| `DATABASE_URL` | PostgreSQL connection string | Auto-configured | ❌ No |
| `RUST_LOG` | Logging level (error/warn/info/debug/trace) | `info` | ❌ No |

### Database Configuration

Database credentials are defined in `docker-compose.yml`:

```yaml
POSTGRES_USER: indexer_user
POSTGRES_PASSWORD: indexer_password
POSTGRES_DB: evm_data_indexer
```

**Security Note:** For production deployments, change these credentials and use Docker secrets or environment variables.

### Port Configuration

Default ports can be changed in `docker-compose.yml`:

```yaml
ports:
  - "5432:5432"  # PostgreSQL
  - "3000:3000"  # API Server
```

---

## 🛠️ Docker Commands

### Using the Management Script

A helper script `docker.sh` is provided for common operations:

```bash
# Start services
./docker.sh start

# Stop services
./docker.sh stop

# View logs
./docker.sh logs

# Restart services
./docker.sh restart

# Rebuild after code changes
./docker.sh rebuild

# Check status
./docker.sh status

# Clean up (removes all data)
./docker.sh clean
```

### Using Docker Compose Directly

```bash
# Start in foreground (see logs in real-time)
docker-compose up

# Start in background
docker-compose up -d

# Stop services
docker-compose down

# View logs
docker-compose logs -f indexer
docker-compose logs -f postgres

# Restart a specific service
docker-compose restart indexer

# Rebuild after code changes
docker-compose up -d --build

# Remove everything including volumes
docker-compose down -v
```

### Useful Docker Commands

```bash
# Execute commands in the indexer container
docker exec -it evm-indexer-app /bin/bash

# Access PostgreSQL directly
docker exec -it evm-indexer-db psql -U indexer_user -d evm_data_indexer

# View resource usage
docker stats

# Inspect container details
docker inspect evm-indexer-app
```

---

## 🔍 Troubleshooting

### Container Won't Start

**Check logs:**
```bash
docker-compose logs indexer
docker-compose logs postgres
```

**Common issues:**
- Missing `.env` file → Copy from `.env.example`
- Invalid `ETH_RPC_URL` → Verify your API key
- Port conflicts → Change ports in `docker-compose.yml`

### Database Connection Errors

**Verify database is healthy:**
```bash
docker-compose ps
```

Look for `healthy` status on the postgres service.

**Manual health check:**
```bash
docker exec evm-indexer-db pg_isready -U indexer_user -d evm_data_indexer
```

### Indexer Not Syncing

**Check RPC connectivity:**
```bash
docker-compose logs indexer | grep "ETH"
```

**Verify environment variables:**
```bash
docker exec evm-indexer-app env | grep ETH_RPC_URL
```

### Reset Database

**Warning: This deletes all indexed data!**

```bash
# Stop services and remove volumes
docker-compose down -v

# Start fresh
docker-compose up -d
```

### View Database Contents

```bash
# Connect to PostgreSQL
docker exec -it evm-indexer-db psql -U indexer_user -d evm_data_indexer

# Check tables
\dt

# View last synced block
SELECT * FROM indexer_status;

# Count indexed blocks
SELECT COUNT(*) FROM blocks;

# Exit
\q
```

---

## 🚀 Advanced Usage

### Production Deployment

For production environments:

1. **Use Docker secrets for sensitive data:**
   ```yaml
   secrets:
     db_password:
       file: ./secrets/db_password.txt
   ```

2. **Enable resource limits:**
   ```yaml
   services:
     indexer:
       deploy:
         resources:
           limits:
             cpus: '2'
             memory: 4G
   ```

3. **Configure logging:**
   ```yaml
   logging:
     driver: "json-file"
     options:
       max-size: "10m"
       max-file: "3"
   ```

### Custom Network Configuration

To integrate with existing Docker networks:

```yaml
networks:
  evm-indexer-network:
    external: true
    name: my-existing-network
```

### Backup and Restore

**Backup database:**
```bash
docker exec evm-indexer-db pg_dump -U indexer_user evm_data_indexer > backup.sql
```

**Restore database:**
```bash
cat backup.sql | docker exec -i evm-indexer-db psql -U indexer_user -d evm_data_indexer
```

### Multi-Chain Deployment

To index multiple chains, create separate compose files:

```bash
# Ethereum Mainnet
docker-compose -f docker-compose.yml up -d

# Polygon
docker-compose -f docker-compose.polygon.yml up -d
```

---

## 📊 Monitoring

### Health Checks

The setup includes automatic health checks:

- **PostgreSQL:** Checks database connectivity every 10 seconds
- **Indexer:** Depends on PostgreSQL health before starting

### Metrics and Logs

**View real-time logs:**
```bash
docker-compose logs -f --tail=100 indexer
```

**Export logs:**
```bash
docker-compose logs indexer > indexer.log
```

---

## 🔒 Security Best Practices

1. **Change default passwords** in production
2. **Use environment-specific `.env` files** (don't commit to git)
3. **Limit exposed ports** (use reverse proxy for API)
4. **Enable SSL/TLS** for database connections
5. **Regular security updates:**
   ```bash
   docker-compose pull
   docker-compose up -d
   ```

---

## 📚 Additional Resources

- [Docker Documentation](https://docs.docker.com/)
- [Docker Compose Reference](https://docs.docker.com/compose/compose-file/)
- [PostgreSQL Docker Hub](https://hub.docker.com/_/postgres)
- [Main README](./README.md)
- [Database Setup Guide](./DB_SETUP.md)

---

## 🤝 Support

If you encounter issues:

1. Check the [Troubleshooting](#troubleshooting) section
2. Review container logs: `docker-compose logs`
3. Open an issue on GitHub with logs and configuration details

---

**Happy Indexing! 🚀**
