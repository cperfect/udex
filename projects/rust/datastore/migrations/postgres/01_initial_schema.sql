-- Initial schema for Udex datastore.
--
-- entry_context merges the former entry + context tables into one.
-- UNIQUE(index_name, context_hash) enforces the 1:1 constraint per index:
-- one context fingerprint produces exactly one entry key within a given index.
-- The same context may appear in different indexes with independent keys.
-- The unique constraint also serves as the B-tree index for context lookups.

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
    index_name      TEXT        NOT NULL REFERENCES index(name) ON DELETE RESTRICT,
    context_hash    TEXT        NOT NULL,
    pairs           JSONB       NOT NULL,
    dek             TEXT,
    kek_id          TEXT,
    hash_algorithm  TEXT        NOT NULL,
    CONSTRAINT uq_entry_context_index_hash UNIQUE (index_name, context_hash)
);
