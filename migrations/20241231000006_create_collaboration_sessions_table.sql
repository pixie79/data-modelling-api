-- Create collaboration_sessions table
CREATE TABLE IF NOT EXISTS collaboration_sessions (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    domain_id UUID REFERENCES domains(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

CREATE INDEX idx_collaboration_sessions_workspace_id ON collaboration_sessions(workspace_id);
CREATE INDEX idx_collaboration_sessions_domain_id ON collaboration_sessions(domain_id);
CREATE INDEX idx_collaboration_sessions_created_by ON collaboration_sessions(created_by);
