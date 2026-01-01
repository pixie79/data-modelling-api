-- Create collaboration_requests table
CREATE TABLE IF NOT EXISTS collaboration_requests (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES collaboration_sessions(id) ON DELETE CASCADE,
    requester_id UUID NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, approved, rejected
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    responded_at TIMESTAMPTZ,
    responder_id UUID
);

CREATE INDEX idx_collaboration_requests_session_id ON collaboration_requests(session_id);
CREATE INDEX idx_collaboration_requests_requester_id ON collaboration_requests(requester_id);
CREATE INDEX idx_collaboration_requests_status ON collaboration_requests(status);

-- Partial unique index for pending requests only
CREATE UNIQUE INDEX idx_collaboration_requests_unique_pending
ON collaboration_requests(session_id, requester_id)
WHERE status = 'pending';
