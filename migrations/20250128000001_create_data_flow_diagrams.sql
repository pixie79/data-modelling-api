-- Create data_flow_diagrams table
CREATE TABLE IF NOT EXISTS data_flow_diagrams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    diagram_data JSONB NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_by UUID NOT NULL, -- User ID (no foreign key - users table doesn't exist)
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(domain_id, name)
);

-- Create index for domain lookups
CREATE INDEX IF NOT EXISTS idx_data_flow_diagrams_domain_id ON data_flow_diagrams(domain_id);

-- Create index for created_by lookups
CREATE INDEX IF NOT EXISTS idx_data_flow_diagrams_created_by ON data_flow_diagrams(created_by);

