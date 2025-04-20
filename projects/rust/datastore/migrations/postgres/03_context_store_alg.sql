ALTER TABLE context ADD COLUMN hash_algorithm TEXT NULL; -- temprorarily nullable until we update existing contexts - we don't want a default - the code must set this explicitly
 
UPDATE context SET hash_algorithm = 'sha1' WHERE hash_algorithm IS NULL; -- default to sha1 for existing contexts

ALTER TABLE context ALTER COLUMN hash_algorithm SET NOT NULL; -- make it non-nullable after update