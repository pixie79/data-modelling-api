-- Create cross_domain_refs table for cross-domain table references
CREATE TABLE IF NOT EXISTS cross_domain_refs (
    id UUID PRIMARY KEY,
    target_domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    source_domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    table_id UUID NOT NULL,
    display_alias VARCHAR(255),
    position_x DOUBLE PRECISION,
    position_y DOUBLE PRECISION,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(target_domain_id, table_id)
);

CREATE INDEX idx_cross_domain_refs_target_domain ON cross_domain_refs(target_domain_id);
CREATE INDEX idx_cross_domain_refs_source_domain ON cross_domain_refs(source_domain_id);
CREATE INDEX idx_cross_domain_refs_table_id ON cross_domain_refs(table_id);
