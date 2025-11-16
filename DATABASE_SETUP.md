# Database Setup Guide

This guide helps you set up PostgreSQL with the pgvector extension for Jamey.

## Quick Start (Windows)

### Option 1: Automated PowerShell Script (Recommended)

```powershell
# Basic usage (will prompt for PostgreSQL password)
powershell -ExecutionPolicy Bypass -File setup-db.ps1

# With PostgreSQL password (no prompt)
powershell -ExecutionPolicy Bypass -File setup-db.ps1 -PostgresPassword "your_postgres_password"

# Custom database name and user
powershell -ExecutionPolicy Bypass -File setup-db.ps1 `
    -DbName "jamey" `
    -DbUser "jamey" `
    -DbPassword "your_db_password" `
    -PostgresUser "postgres" `
    -PostgresPassword "your_postgres_password"
```

### Option 2: Manual Setup

1. **Install PostgreSQL** (if not already installed)
   - Download from: https://www.postgresql.org/download/windows/
   - During installation, note your PostgreSQL password

2. **Start PostgreSQL Service**
   - Open Services (Win+R → `services.msc`)
   - Find "postgresql-x64-XX" service
   - Ensure it's running

3. **Create Database and User**
   ```powershell
   # Set PostgreSQL password (replace with your actual password)
   $env:PGPASSWORD = "your_postgres_password"
   
   # Create user
   psql -U postgres -c "CREATE USER jamey WITH PASSWORD 'change_me_in_production';"
   
   # Create database
   psql -U postgres -c "CREATE DATABASE jamey WITH OWNER jamey;"
   
   # Install pgvector extension
   psql -U postgres -d jamey -c "CREATE EXTENSION IF NOT EXISTS vector;"
   
   # Clean up
   Remove-Item Env:\PGPASSWORD
   ```

## Troubleshooting

### Issue: "psql: error: connection to server failed"

**Solution:**
- Ensure PostgreSQL service is running
- Check if PostgreSQL is listening on port 5432
- Verify your PostgreSQL installation path is in your system PATH

### Issue: "extension 'vector' does not exist"

**Solution:**
The pgvector extension needs to be built and installed. You're likely building it from source.

1. **Build pgvector extension:**
   ```powershell
   cd C:\Users\JAMEYMILNER\Downloads\pgvector
   .\build.bat
   ```

2. **Install the extension:**
   ```powershell
   # Copy the built extension to PostgreSQL
   # The build.bat should output the location, typically:
   # C:\Program Files\PostgreSQL\18\lib\vector.dll
   # C:\Program Files\PostgreSQL\18\share\extension\vector.control
   ```

3. **Verify installation:**
   ```powershell
   psql -U postgres -d jamey -c "CREATE EXTENSION vector;"
   ```

### Issue: "Freezing when creating database"

**Common causes:**
1. **Password prompt hanging** - Use the PowerShell script with `-PostgresPassword` parameter
2. **PostgreSQL service not running** - Start the service from Services panel
3. **Connection timeout** - Check firewall settings
4. **pgvector extension missing** - Build and install pgvector first (see above)

### Issue: "permission denied to create extension"

**Solution:**
You need superuser privileges to create extensions. Use the `postgres` superuser:

```powershell
psql -U postgres -d jamey -c "CREATE EXTENSION IF NOT EXISTS vector;"
```

## Verification

After setup, verify everything works:

```powershell
# Test connection
psql -U jamey -d jamey -c "SELECT version();"

# Verify pgvector extension
psql -U jamey -d jamey -c "SELECT * FROM pg_extension WHERE extname = 'vector';"

# Test vector type
psql -U jamey -d jamey -c "SELECT '[1,2,3]'::vector;"
```

## Configuration

Update your `.env` or `config/windows.env` file:

```env
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_DB=jamey
POSTGRES_USER=jamey
POSTGRES_PASSWORD=change_me_in_production
POSTGRES_MAX_CONNECTIONS=10
```

## Next Steps

1. Update your environment configuration with database credentials
2. Run `cargo run` to start the application
3. The application will automatically create the required tables on first run

## Notes

- The `PostgresMemoryStore::new()` function now automatically creates the `vector` extension if it doesn't exist
- This prevents freezing when the extension is missing
- Always use strong passwords in production environments

