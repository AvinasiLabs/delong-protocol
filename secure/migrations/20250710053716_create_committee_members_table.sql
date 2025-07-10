-- Add migration script here
CREATE TABLE IF NOT EXISTS committee_members (
    id SERIAL PRIMARY KEY,
    member_wallet VARCHAR(255) NOT NULL,
    is_approved BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_committee_members_member_wallet UNIQUE (member_wallet)
);

CREATE INDEX IF NOT EXISTS idx_committee_members_is_approved ON committee_members(is_approved);
CREATE INDEX IF NOT EXISTS idx_committee_members_created_at ON committee_members(created_at); 