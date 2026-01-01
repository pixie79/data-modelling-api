-- Create tables table (for storing table definitions)
CREATE TABLE IF NOT EXISTS tables (
    id UUID PRIMARY KEY,
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    data JSONB NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_by UUID NOT NULL,
    updated_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(domain_id, name)
);

CREATE INDEX idx_tables_domain_id ON tables(domain_id);
CREATE INDEX idx_tables_name ON tables(name);
CREATE INDEX idx_tables_created_by ON tables(created_by);
