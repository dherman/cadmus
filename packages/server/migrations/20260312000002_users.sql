-- Users table
CREATE TABLE users (
    id              UUID PRIMARY KEY,
    email           TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    password_hash   TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);

-- Add owner tracking to documents
ALTER TABLE documents ADD COLUMN created_by UUID REFERENCES users(id);

-- Add FK constraint to document_permissions (column exists but had no FK)
ALTER TABLE document_permissions
    ADD CONSTRAINT fk_document_permissions_user_id
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
