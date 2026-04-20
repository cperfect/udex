-- Rename stored 'sha1' hash_algorithm values to 'Xxh3' following the
-- replacement of HashAlgorithm::Sha1 with HashAlgorithm::Xxh3.
-- Migration 03 seeded context rows with 'sha1'; new rows use 'Xxh3'
-- (the string produced by HashAlgorithm::Xxh3.as_str_name()).

UPDATE index   SET hash_algorithm = 'Xxh3' WHERE hash_algorithm IN ('sha1', 'Sha1', 'SHA1');
UPDATE context SET hash_algorithm = 'Xxh3' WHERE hash_algorithm IN ('sha1', 'Sha1', 'SHA1');
