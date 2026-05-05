#!/bin/bash
set -e

# This script creates a second user and database for Hydra
# The primary user/db are still created by the environment variables in docker-compose
if [ -z "${HYDRA_DB_PASSWORD_SECRET}" ]; then
  echo "ERROR: HYDRA_DB_PASSWORD_SECRET is not set" >&2
  exit 1
fi

# Pass the password as a named psql variable and reference it with :'hydra_pw' (note
# the colon-quote syntax). This lets psql handle quoting and escaping, so the password
# is safe regardless of its character content. Direct ${VAR} interpolation into a SQL
# heredoc is fragile — a single quote in the value would break the literal or, worse,
# execute injected DDL.
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "postgres" \
     -v "hydra_pw=${HYDRA_DB_PASSWORD_SECRET}" \
     <<-EOSQL
    CREATE USER hydra WITH PASSWORD :'hydra_pw';
    CREATE DATABASE hydra WITH OWNER hydra;
    GRANT ALL PRIVILEGES ON DATABASE hydra TO hydra;
EOSQL
