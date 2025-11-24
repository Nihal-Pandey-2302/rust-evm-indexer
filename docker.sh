#!/bin/bash
# Docker management script for EVM Indexer

set -e

# Detect docker-compose command (supports both v1 and v2)
if command -v docker-compose &> /dev/null; then
    DOCKER_COMPOSE="docker-compose"
elif docker compose version &> /dev/null; then
    DOCKER_COMPOSE="docker compose"
else
    echo "Error: Neither 'docker-compose' nor 'docker compose' is available"
    echo "Please install Docker Compose: https://docs.docker.com/compose/install/"
    exit 1
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}ℹ $1${NC}"
}

# Check if .env exists
check_env() {
    if [ ! -f .env ]; then
        print_error ".env file not found!"
        print_info "Creating .env from .env.example..."
        cp .env.example .env
        print_info "Please edit .env and add your ETH_RPC_URL"
        exit 1
    fi
}

# Start services
start() {
    print_info "Starting EVM Indexer services..."
    check_env
    $DOCKER_COMPOSE up -d
    print_success "Services started!"
    print_info "API Server: http://localhost:3000"
    print_info "Swagger UI: http://localhost:3000/swagger-ui"
}

# Stop services
stop() {
    print_info "Stopping EVM Indexer services..."
    $DOCKER_COMPOSE down
    print_success "Services stopped!"
}

# View logs
logs() {
    $DOCKER_COMPOSE logs -f indexer
}

# Restart services
restart() {
    print_info "Restarting EVM Indexer services..."
    $DOCKER_COMPOSE restart
    print_success "Services restarted!"
}

# Rebuild and restart
rebuild() {
    print_info "Rebuilding and restarting services..."
    $DOCKER_COMPOSE up -d --build
    print_success "Services rebuilt and restarted!"
}

# Clean up (remove volumes)
clean() {
    print_info "Stopping services and removing volumes..."
    $DOCKER_COMPOSE down -v
    print_success "Cleanup complete!"
}

# Show status
status() {
    $DOCKER_COMPOSE ps
}

# Show help
help() {
    echo "EVM Indexer Docker Management Script"
    echo ""
    echo "Usage: ./docker.sh [command]"
    echo ""
    echo "Commands:"
    echo "  start     - Start all services"
    echo "  stop      - Stop all services"
    echo "  restart   - Restart all services"
    echo "  rebuild   - Rebuild and restart services"
    echo "  logs      - View indexer logs (follow mode)"
    echo "  status    - Show service status"
    echo "  clean     - Stop services and remove all data"
    echo "  help      - Show this help message"
}

# Main script logic
case "$1" in
    start)
        start
        ;;
    stop)
        stop
        ;;
    restart)
        restart
        ;;
    rebuild)
        rebuild
        ;;
    logs)
        logs
        ;;
    status)
        status
        ;;
    clean)
        clean
        ;;
    help|--help|-h|"")
        help
        ;;
    *)
        print_error "Unknown command: $1"
        help
        exit 1
        ;;
esac
