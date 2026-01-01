-- Create collaboration_participants table
CREATE TABLE IF NOT EXISTS collaboration_participants (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES collaboration_sessions(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    permission VARCHAR(50) NOT NULL DEFAULT 'viewer', -- viewer, editor, owner
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_online BOOLEAN NOT NULL DEFAULT false,
    cursor_x DOUBLE PRECISION,
    cursor_y DOUBLE PRECISION,
    selected_tables JSONB DEFAULT '[]'::jsonb,
    selected_relationships JSONB DEFAULT '[]'::jsonb,
    editing_table UUID,
    UNIQUE(session_id, user_id)
);

CREATE INDEX idx_collaboration_participants_session_id ON collaboration_participants(session_id);
CREATE INDEX idx_collaboration_participants_user_id ON collaboration_participants(user_id);
CREATE INDEX idx_collaboration_participants_is_online ON collaboration_participants(is_online);
