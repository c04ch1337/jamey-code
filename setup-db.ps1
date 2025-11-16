# PowerShell script to set up PostgreSQL database for Jamey on Windows
# This script handles Windows-specific database setup

param(
    [string]$DbName = "jamey_production",
    [string]$DbUser = "jamey",
    [string]$DbPassword = "change_me_in_production",
    [string]$PostgresUser = "postgres",
    [string]$PostgresPassword = ""
)

Write-Host "Setting up PostgreSQL database for Jamey..." -ForegroundColor Cyan

# Check if psql is available
if (-not (Get-Command psql -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: psql not found. Please ensure PostgreSQL is installed and in PATH." -ForegroundColor Red
    Write-Host "Download from: https://www.postgresql.org/download/windows/" -ForegroundColor Yellow
    exit 1
}

# Set PGPASSWORD environment variable if provided
if ($PostgresPassword) {
    $env:PGPASSWORD = $PostgresPassword
    Write-Host "Using provided PostgreSQL password for authentication" -ForegroundColor Green
} else {
    Write-Host "Note: You may be prompted for PostgreSQL password" -ForegroundColor Yellow
}

# Create user if it doesn't exist
Write-Host "Creating database user '$DbUser'..." -ForegroundColor Cyan
$createUserQuery = @"
DO `$`$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_user WHERE usename = '$DbUser') THEN
        CREATE USER $DbUser WITH PASSWORD '$DbPassword';
    ELSE
        ALTER USER $DbUser WITH PASSWORD '$DbPassword';
    END IF;
END
`$`$;
"@

try {
    $createUserResult = $createUserQuery | psql -U $PostgresUser -d postgres -q 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Warning: User creation may have failed. Error: $createUserResult" -ForegroundColor Yellow
    } else {
        Write-Host "User '$DbUser' created/updated successfully" -ForegroundColor Green
    }
} catch {
    Write-Host "Error creating user: $_" -ForegroundColor Red
    exit 1
}

# Create database if it doesn't exist
Write-Host "Creating database '$DbName'..." -ForegroundColor Cyan
$createDbQuery = "SELECT 1 FROM pg_database WHERE datname = '$DbName'"

try {
    $dbExists = psql -U $PostgresUser -d postgres -tAc $createDbQuery 2>&1
    if ($dbExists -match "1") {
        Write-Host "Database '$DbName' already exists" -ForegroundColor Yellow
    } else {
        $createDbResult = psql -U $PostgresUser -d postgres -c "CREATE DATABASE $DbName WITH OWNER $DbUser;" 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Error creating database: $createDbResult" -ForegroundColor Red
            exit 1
        }
        Write-Host "Database '$DbName' created successfully" -ForegroundColor Green
    }
} catch {
    Write-Host "Error checking/creating database: $_" -ForegroundColor Red
    exit 1
}

# Install pgvector extension
Write-Host "Installing pgvector extension..." -ForegroundColor Cyan
try {
    $extensionResult = psql -U $PostgresUser -d $DbName -c "CREATE EXTENSION IF NOT EXISTS vector;" 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "WARNING: pgvector extension installation failed!" -ForegroundColor Red
        Write-Host "Error: $extensionResult" -ForegroundColor Red
        Write-Host "" -ForegroundColor Yellow
        Write-Host "You may need to:" -ForegroundColor Yellow
        Write-Host "1. Build and install pgvector extension manually" -ForegroundColor Yellow
        Write-Host "2. Ensure PostgreSQL has extension installation privileges" -ForegroundColor Yellow
        Write-Host "3. Check that pgvector is compiled for your PostgreSQL version" -ForegroundColor Yellow
        exit 1
    } else {
        Write-Host "pgvector extension installed successfully" -ForegroundColor Green
    }
} catch {
    Write-Host "Error installing extension: $_" -ForegroundColor Red
    exit 1
}

# Grant privileges
Write-Host "Granting privileges..." -ForegroundColor Cyan
try {
    psql -U $PostgresUser -d $DbName -c "GRANT ALL PRIVILEGES ON DATABASE $DbName TO $DbUser;" | Out-Null
    psql -U $PostgresUser -d $DbName -c "GRANT ALL ON SCHEMA public TO $DbUser;" | Out-Null
    Write-Host "Privileges granted successfully" -ForegroundColor Green
} catch {
    Write-Host "Warning: Could not grant privileges: $_" -ForegroundColor Yellow
}

# Clean up password from environment
if ($PostgresPassword) {
    Remove-Item Env:\PGPASSWORD
}

Write-Host "" -ForegroundColor Green
Write-Host "Database setup complete!" -ForegroundColor Green
Write-Host "Database: $DbName" -ForegroundColor Cyan
Write-Host "User: $DbUser" -ForegroundColor Cyan
Write-Host "" -ForegroundColor Green
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Update your .env file with these database credentials" -ForegroundColor White
Write-Host "2. Run 'cargo run' to start the application" -ForegroundColor White

