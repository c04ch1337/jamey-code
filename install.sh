#!/bin/bash
set -e

echo "Installing Digital Twin Jamey..."

# Check if running on Windows
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    echo "Windows system detected"
    WINDOWS=true
else
    WINDOWS=false
fi

# Check for Rust installation
if ! command -v rustc &> /dev/null; then
    echo "Rust not found. Installing Rust..."
    if [ "$WINDOWS" = true ]; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    else
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    fi
    source $HOME/.cargo/env
fi

# Check for PostgreSQL
if ! command -v psql &> /dev/null; then
    echo "PostgreSQL not found. Please install PostgreSQL first:"
    if [ "$WINDOWS" = true ]; then
        echo "Download from: https://www.postgresql.org/download/windows/"
    else
        echo "Linux: sudo apt-get install postgresql postgresql-contrib"
        echo "macOS: brew install postgresql"
    fi
    exit 1
fi

# Create .env.local if it doesn't exist
if [ ! -f .env.local ]; then
    echo "Creating .env.local from example..."
    cp .env.local.example .env.local
    echo "Please update .env.local with your configuration"
fi

# Create necessary directories
echo "Creating project directories..."
mkdir -p backups

# Initialize PostgreSQL database
echo "Setting up PostgreSQL database..."
if [ "$WINDOWS" = true ]; then
    echo "Windows detected. Using PowerShell script for database setup..."
    echo "Please run: powershell -ExecutionPolicy Bypass -File setup-db.ps1"
    echo "Or manually run setup-db.ps1 with appropriate parameters"
    echo ""
    echo "For automated setup, you can also use:"
    echo "  powershell -ExecutionPolicy Bypass -File setup-db.ps1 -PostgresPassword 'your_postgres_password'"
    exit 0
else
    PSQL="sudo -u postgres psql"
fi

# Source .env.local for database configuration
if [ -f .env.local ]; then
    source .env.local
fi

DB_NAME=${POSTGRES_DB:-jamey}
DB_USER=${POSTGRES_USER:-jamey}
DB_PASS=${POSTGRES_PASSWORD:-change_me_in_production}

$PSQL <<EOF
DO \$\$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_user WHERE usename = '$DB_USER') THEN
        CREATE USER $DB_USER WITH PASSWORD '$DB_PASS';
    END IF;
END
\$\$;

CREATE DATABASE $DB_NAME WITH OWNER $DB_USER;
EOF

# Install pgvector extension
$PSQL -d $DB_NAME <<EOF
CREATE EXTENSION IF NOT EXISTS vector;
EOF

echo "Building project..."
cargo build

echo "Installation complete!"
echo "Next steps:"
echo "1. Update .env.local with your configuration"
echo "2. Run 'cargo run' to start the application"