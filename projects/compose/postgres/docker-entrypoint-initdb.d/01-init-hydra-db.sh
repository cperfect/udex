#!/bin/bash
set -e

# This script creates a second user and database for Hydra
# The primary user/db are still created by the environment variables in docker-compose
if [ -z "${HYDRA_DB_PASSWORD_SECRET}" ]; then
  echo "ERROR: HYDRA_DB_PASSWORD_SECRET is not set" >&2
  exit 1
fi

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "postgres" <<-EOSQL
    CREATE USER hydra WITH PASSWORD '${HYDRA_DB_PASSWORD_SECRET}';
    CREATE DATABASE hydra WITH OWNER hydra;
    GRANT ALL PRIVILEGES ON DATABASE hydra TO hydra;
EOSQL
