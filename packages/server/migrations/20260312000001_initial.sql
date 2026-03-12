-- Documents table: metadata for each document
CREATE TABLE documents (
    id              UUID PRIMARY KEY,
    title           TEXT NOT NULL,
    schema_version  INTEGER NOT NULL DEFAULT 1,
    snapshot_key    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Document permissions: created now, enforced in Milestone 3
CREATE TABLE document_permissions (
    id              UUID PRIMARY KEY,
    document_id     UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL,
    role            TEXT NOT NULL CHECK (role IN ('read', 'comment', 'edit')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_document_permissions_document_id ON document_permissions(document_id);
CREATE INDEX idx_document_permissions_user_id ON document_permissions(user_id);

-- Update log: append-only log of Yrs updates between snapshot compactions
CREATE TABLE update_log (
    id              BIGSERIAL PRIMARY KEY,
    document_id     UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    data            BYTEA NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_update_log_document_id ON update_log(document_id, id);
