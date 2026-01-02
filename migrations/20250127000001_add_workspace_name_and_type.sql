-- Add name and type columns to workspaces table
-- Name must be unique per email address
-- Type can be 'personal' or 'organization' and can change over time

ALTER TABLE workspaces
ADD COLUMN IF NOT EXISTS name VARCHAR(255),
ADD COLUMN IF NOT EXISTS type VARCHAR(50) DEFAULT 'personal' CHECK (type IN ('personal', 'organization'));

-- Create unique index on (email, name) to enforce uniqueness per email
CREATE UNIQUE INDEX IF NOT EXISTS idx_workspaces_email_name ON workspaces(email, name);

-- Update existing workspaces to have default name based on email if name is NULL
UPDATE workspaces
SET name = COALESCE(name, 'Workspace ' || SUBSTRING(email FROM 1 FOR POSITION('@' IN email) - 1))
WHERE name IS NULL;

-- Make name NOT NULL after setting defaults
ALTER TABLE workspaces
ALTER COLUMN name SET NOT NULL;

