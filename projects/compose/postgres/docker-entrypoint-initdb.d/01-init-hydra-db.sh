#!/bin/bash
set -e

# This script creates a second user and database for Hydra
# The primary user/db are still created by the environment variables in docker-compose
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "postgres" <<-EOSQL
    CREATE USER hydra WITH PASSWORD 'secret';
    CREATE DATABASE hydra WITH OWNER hydra;
    GRANT ALL PRIVILEGES ON DATABASE hydra TO hydra;
EOSQL
