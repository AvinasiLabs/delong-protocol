#!/bin/bash

# Secure Service Development Database Management Script
# This script helps manage the PostgreSQL database for local development

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory and paths
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"  # secure directory
DEPLOY_DIR="$PROJECT_DIR"
MIGRATIONS_DIR="$PROJECT_DIR/migrations"

# Database connection details
DB_USER="secure_user"
DB_PASS="secure_dev_password"
DB_NAME="secure_db"
DB_HOST="localhost"
DB_PORT="5432"

# Export DATABASE_URL for sqlx
export DATABASE_URL="postgresql://${DB_USER}:${DB_PASS}@${DB_HOST}:${DB_PORT}/${DB_NAME}"

# Function to print colored output
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_header() {
    echo -e "\n${BLUE}==== $1 ====${NC}"
}

# Check if Docker is installed and running
check_docker() {
    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed. Please install Docker first."
        exit 1
    fi

    if ! command -v docker-compose &> /dev/null; then
        print_error "docker-compose is not installed. Please install docker-compose first."
        exit 1
    fi

    # Check if Docker daemon is running
    if ! docker info &> /dev/null; then
        print_error "Docker daemon is not running. Please start Docker."
        exit 1
    fi
}

# Check if required files exist
check_files() {
    if [ ! -f "$DEPLOY_DIR/docker-compose.dev.yml" ]; then
        print_error "docker-compose.dev.yml not found in $DEPLOY_DIR"
        exit 1
    fi
}

# Start the database
start_db() {
    print_header "Starting PostgreSQL Database"

    # Create .env file if it doesn't exist
    if [ ! -f "$PROJECT_DIR/.env" ]; then
        if [ -f "$PROJECT_DIR/.env.development" ]; then
            print_info "Creating .env file from .env.development..."
            cp "$PROJECT_DIR/.env.development" "$PROJECT_DIR/.env"
        else
            print_warning ".env.development not found. Please configure .env manually."
        fi
    fi

    # Start containers
    cd "$DEPLOY_DIR"
    docker-compose -f docker-compose.dev.yml up -d postgres redis

    # Wait for database to be ready
    print_info "Waiting for database to be ready..."
    local max_attempts=30
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        if docker-compose -f docker-compose.dev.yml exec -T postgres pg_isready -U $DB_USER -d $DB_NAME &> /dev/null; then
            print_info "Database is ready!"
            print_info "Connection string: $DATABASE_URL"
            break
        fi

        attempt=$((attempt + 1))
        if [ $attempt -eq $max_attempts ]; then
            print_error "Database failed to start after $max_attempts attempts"
            exit 1
        fi

        echo -n "."
        sleep 1
    done
    echo
}

# Stop the database
stop_db() {
    print_header "Stopping Database Services"

    cd "$DEPLOY_DIR"
    docker-compose -f docker-compose.dev.yml stop postgres redis pgadmin

    print_info "Database services stopped."
}

# Restart the database
restart_db() {
    print_header "Restarting Database Services"
    stop_db
    sleep 2
    start_db
}

# Clean the database (removes all data)
clean_db() {
    print_header "Clean Database"
    print_warning "This will remove ALL database data and volumes!"
    print_warning "Are you sure you want to continue? (yes/no)"

    read -r response
    if [[ "$response" != "yes" ]]; then
        print_info "Operation cancelled."
        return
    fi

    cd "$DEPLOY_DIR"
    print_info "Removing containers and volumes..."
    docker-compose -f docker-compose.dev.yml down -v

    print_info "Database cleaned successfully."
}

# Show database logs
logs_db() {
    print_header "Database Logs"
    cd "$DEPLOY_DIR"
    docker-compose -f docker-compose.dev.yml logs -f postgres
}

# Initialize migrations
init_migrations() {
    print_header "Initialize SQLx Migrations"

    cd "$PROJECT_DIR"

    if [ ! -d "migrations" ]; then
        print_info "Creating migrations directory..."

        # Check if sqlx is installed
        if ! command -v sqlx &> /dev/null; then
            print_error "sqlx-cli is not installed."
            print_info "Install it with: cargo install sqlx-cli --no-default-features --features postgres"
            exit 1
        fi

        print_info "Creating initial migration..."
        sqlx migrate add -r initial_schema

        print_info "Initial migration created!"
        print_info "Migration files:"
        ls -la migrations/

        print_info ""
        print_info "Next steps:"
        print_info "1. Edit the migration files in the migrations/ directory"
        print_info "2. Add your schema definitions to the .up.sql file"
        print_info "3. Add rollback statements to the .down.sql file"
        print_info "4. Run './scripts/dev-db.sh migrate' to apply migrations"
        print_info ""
        print_info "Note: For quick development, init-db.sql is already applied automatically."
    else
        print_info "Migrations directory already exists."
        print_info "Current migrations:"
        ls -la "$MIGRATIONS_DIR"
    fi
}

# Run migrations
run_migrations() {
    print_header "Running SQLx Migrations"

    # Ensure database is running
    cd "$DEPLOY_DIR"
    if ! docker-compose -f docker-compose.dev.yml ps | grep -q "secure-postgres-dev.*Up"; then
        print_info "Database is not running. Starting it first..."
        cd "$PROJECT_DIR"
        start_db
    fi

    cd "$PROJECT_DIR"

    if [ ! -d "migrations" ]; then
        print_warning "No migrations directory found."
        print_info "The database was initialized with init-db.sql (dev environment only)."
        print_info "For production-grade migrations, run: ./scripts/dev-db.sh init-migrations"
        return 0
    fi

    print_info "Running migrations..."
    if ! command -v sqlx &> /dev/null; then
        print_error "sqlx-cli is not installed."
        print_info "Install it with: cargo install sqlx-cli --no-default-features --features postgres"
        exit 1
    fi

    sqlx migrate run

    print_info "Migrations completed successfully."
}

# Revert last migration
revert_migration() {
    print_header "Reverting Last Migration"

    cd "$PROJECT_DIR"

    if [ ! -d "migrations" ]; then
        print_error "No migrations directory found."
        return 1
    fi

    print_info "Reverting last migration..."
    sqlx migrate revert

    print_info "Migration reverted successfully."
}

# Connect to database with psql
connect_db() {
    print_header "Connecting to Database"

    cd "$DEPLOY_DIR"
    print_info "Connecting as user: $DB_USER to database: $DB_NAME"
    docker-compose -f docker-compose.dev.yml exec postgres psql -U $DB_USER -d $DB_NAME
}

# Show database status
show_status() {
    print_header "Database Status"

    cd "$DEPLOY_DIR"

    if docker-compose -f docker-compose.dev.yml ps | grep -q "secure-postgres-dev.*Up"; then
        print_info "PostgreSQL is ${GREEN}running${NC}"
    else
        print_warning "PostgreSQL is ${RED}not running${NC}"
    fi

    if docker-compose -f docker-compose.dev.yml ps | grep -q "secure-redis-dev.*Up"; then
        print_info "Redis is ${GREEN}running${NC}"
    else
        print_warning "Redis is ${RED}not running${NC}"
    fi

    if docker-compose -f docker-compose.dev.yml ps | grep -q "secure-pgadmin-dev.*Up"; then
        print_info "pgAdmin is ${GREEN}running${NC} at http://localhost:5050"
        print_info "  Email: admin@secure.local"
        print_info "  Password: pgadmin_password"
    else
        print_info "pgAdmin is ${YELLOW}not running${NC}"
    fi

    echo
    docker-compose -f docker-compose.dev.yml ps

    print_info ""
    print_info "Database URL: $DATABASE_URL"

    # Check migrations status
    cd "$PROJECT_DIR"
    if [ -d "migrations" ]; then
        print_info ""
        print_info "Migrations: ${GREEN}Initialized${NC}"
        if command -v sqlx &> /dev/null 2>&1; then
            print_info "Pending migrations:"
            sqlx migrate info || true
        fi
    else
        print_info ""
        print_info "Migrations: ${YELLOW}Not initialized${NC}"
        print_info "Using init-db.sql for development"
    fi
}

# Start pgAdmin
start_pgadmin() {
    print_header "Starting pgAdmin"

    cd "$DEPLOY_DIR"
    docker-compose -f docker-compose.dev.yml up -d pgadmin

    print_info "pgAdmin started at http://localhost:5050"
    print_info "Login credentials:"
    print_info "  Email: admin@secure.local"
    print_info "  Password: pgadmin_password"
}

# Export database schema
export_schema() {
    print_header "Exporting Database Schema"

    local timestamp=$(date +%Y%m%d_%H%M%S)
    local output_file="$PROJECT_DIR/schema_export_${timestamp}.sql"

    cd "$DEPLOY_DIR"
    docker-compose -f docker-compose.dev.yml exec -T postgres pg_dump \
        -U $DB_USER -d $DB_NAME --schema-only --no-owner --no-privileges > "$output_file"

    print_info "Schema exported to: $output_file"
}

# Show schema management info
schema_info() {
    print_header "Schema Management Information"

    print_info "This project supports two approaches for database schema management:"
    echo
    echo "1. ${BLUE}Quick Development Setup${NC} (current default)"
    echo "   - Uses deploy/init-db.sql"
    echo "   - Applied automatically when starting PostgreSQL container"
    echo "   - Good for quick development environment setup"
    echo "   - Not suitable for production"
    echo
    echo "2. ${BLUE}Production-Grade Migrations${NC}"
    echo "   - Uses sqlx migrations in migrations/ directory"
    echo "   - Version controlled, incremental changes"
    echo "   - Can be rolled back"
    echo "   - Required for production deployments"
    echo
    echo "To switch to migrations:"
    echo "  1. Run: ./scripts/dev-db.sh init-migrations"
    echo "  2. Copy schema from deploy/init-db.sql to migrations"
    echo "  3. Run: ./scripts/dev-db.sh migrate"
}

# Show help
show_help() {
    echo "Secure Service Database Management Script"
    echo
    echo "Usage: $0 [command]"
    echo
    echo "Basic Commands:"
    echo "  start           Start PostgreSQL and Redis"
    echo "  stop            Stop all database services"
    echo "  restart         Restart database services"
    echo "  status          Show service status"
    echo "  logs            Show PostgreSQL logs (follow mode)"
    echo "  connect         Connect to database with psql"
    echo "  clean           Remove all data and volumes (DESTRUCTIVE!)"
    echo
    echo "Schema Management:"
    echo "  schema-info     Show schema management approaches"
    echo "  init-migrations Initialize sqlx migrations (for production)"
    echo "  migrate         Run database migrations"
    echo "  revert          Revert last migration"
    echo "  export-schema   Export current database schema"
    echo
    echo "Additional Services:"
    echo "  pgadmin         Start pgAdmin web interface"
    echo
    echo "Environment:"
    echo "  Database URL: $DATABASE_URL"
    echo "  pgAdmin URL:  http://localhost:5050"
    echo
    echo "Examples:"
    echo "  $0 start        # Start the database (uses init-db.sql)"
    echo "  $0 status       # Check what's running"
    echo "  $0 connect      # Connect with psql client"
}

# Main script logic
main() {
    check_docker
    check_files

    case "${1:-help}" in
        start)
            start_db
            ;;
        stop)
            stop_db
            ;;
        restart)
            restart_db
            ;;
        status)
            show_status
            ;;
        logs)
            logs_db
            ;;
        connect)
            connect_db
            ;;
        clean)
            clean_db
            ;;
        schema-info)
            schema_info
            ;;
        init-migrations)
            init_migrations
            ;;
        migrate)
            run_migrations
            ;;
        revert)
            revert_migration
            ;;
        pgadmin)
            start_pgadmin
            ;;
        export-schema)
            export_schema
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            print_error "Unknown command: $1"
            echo
            show_help
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
