-- Rename langid enum value 'ngerman' to 'german'
ALTER TYPE langid RENAME VALUE 'ngerman' TO 'german';

-- Create license enum type
CREATE TYPE license AS ENUM (
    'cc-by-3',
    'cc-by-4',
    'cc-by-sa-3',
    'cc-by-sa-4',
    'cc-by-nc-3',
    'cc-by-nc-4',
    'cc-by-nc-sa-3',
    'cc-by-nc-sa-4',
    'cc-by-nd-4',
    'cc-by-nc-nd-4',
    'cc0',
    'all-rights-reserved'
);

-- Convert license column from TEXT to the new enum type
ALTER TABLE bibitems
    ALTER COLUMN license TYPE license USING license::license;
