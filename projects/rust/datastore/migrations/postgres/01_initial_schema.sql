-- Initial schema for Udex datastore.
--
-- entry_context merges the former entry + context tables into one.
-- UNIQUE(context_hash) enforces the 1:1 constraint at the database level:
-- one context fingerprint produces exactly one entry key per index.

CREATE TABLE IF NOT EXISTS index (
    name                    TEXT        PRIMARY KEY,
    description             TEXT        NOT NULL,
    max_bulk_operations     INTEGER     NOT NULL,
    max_key_length          INTEGER     NOT NULL,
    max_value_length        INTEGER     NOT NULL,
    max_kv_pairs_per_context INTEGER    NOT NULL,
    hash_algorithm          TEXT        NOT NULL,
    created_at              TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    created_by              TEXT        NOT NULL,
    updated_at              TIMESTAMPTZ,
    updated_by              TEXT
);

CREATE TABLE IF NOT EXISTS entry_context (
    key             UUID        PRIMARY KEY,
    index_name      TEXT        NOT NULL REFERENCES index(name),
    context_hash    TEXT        NOT NULL UNIQUE,
    pairs           JSONB       NOT NULL,
    dek             TEXT,
    kek_id          TEXT,
    hash_algorithm  TEXT        NOT NULL
);

-- Composite index for index-scoped context lookups.
-- Also serves index_name-only queries via the leftmost prefix.
CREATE INDEX idx_entry_context_index_context
    ON entry_context (index_name, context_hash);
