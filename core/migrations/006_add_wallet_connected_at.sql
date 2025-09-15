-- Migration: Add wallet_connected_at field to users table
-- Purpose: Track when a wallet address was linked to a user account via SIWE

-- Add wallet_connected_at column to users table
ALTER TABLE users
ADD COLUMN IF NOT EXISTS wallet_connected_at TIMESTAMP WITH TIME ZONE;

-- Add comment to the column for documentation
COMMENT ON COLUMN users.wallet_connected_at IS 'Timestamp when the wallet address was linked to this account via SIWE verification';

-- Update existing users with wallet addresses to have a connected_at timestamp
-- (Set to current timestamp for existing wallet addresses, assuming they were connected before this migration)
UPDATE users
SET wallet_connected_at = CURRENT_TIMESTAMP
WHERE wallet_address IS NOT NULL
  AND wallet_connected_at IS NULL;

-- Optional: Add an index if we need to query by wallet connection time
-- CREATE INDEX IF NOT EXISTS idx_users_wallet_connected_at ON users(wallet_connected_at);
