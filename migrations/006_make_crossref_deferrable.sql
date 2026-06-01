-- The crossref column references bibitems(bibkey) in the same table.
-- During bulk import, all bibitems are inserted in one statement — forward
-- references (child before parent) violate the FK at insert time.
-- Making it DEFERRABLE INITIALLY DEFERRED moves the check to COMMIT,
-- when all rows already exist.

ALTER TABLE bibitems
    DROP CONSTRAINT bibitems_crossref_fkey;

ALTER TABLE bibitems
    ADD CONSTRAINT bibitems_crossref_fkey
    FOREIGN KEY (crossref) REFERENCES bibitems(bibkey)
    DEFERRABLE INITIALLY DEFERRED;
