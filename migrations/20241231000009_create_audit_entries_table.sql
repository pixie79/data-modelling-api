-- Create audit_entries table for audit trail
CREATE TABLE IF NOT EXISTS audit_entries (
    id UUID PRIMARY KEY,
    entity_type VARCHAR(50) NOT NULL, -- workspace, domain, table, relationship
    entity_id UUID NOT NULL,
    workspace_id UUID REFERENCES workspaces(id) ON DELETE CASCADE,
    domain_id UUID REFERENCES domains(id) ON DELETE CASCADE,
    action VARCHAR(50) NOT NULL, -- create, update, delete
    user_id UUID NOT NULL,
    user_email VARCHAR(255) NOT NULL,
    changes JSONB,
    previous_data JSONB,
    new_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_entries_entity ON audit_entries(entity_type, entity_id);
CREATE INDEX idx_audit_entries_workspace_id ON audit_entries(workspace_id);
CREATE INDEX idx_audit_entries_domain_id ON audit_entries(domain_id);
CREATE INDEX idx_audit_entries_user_id ON audit_entries(user_id);
CREATE INDEX idx_audit_entries_created_at ON audit_entries(created_at DESC);
