-- Core types and functions that other tables depend on

-- Create enum types (with safe creation)
DO $$ BEGIN
    CREATE TYPE review_status AS ENUM ('reviewing', 'approved', 'rejected');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE execution_status AS ENUM ('queued', 'running', 'completed', 'failed');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE transaction_status AS ENUM ('pending', 'confirmed', 'failed');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Entity type enum for transaction tracking
DO $$ BEGIN
    CREATE TYPE entity_type AS ENUM ('dataset', 'algorithm', 'execution', 'vote', 'committee');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Function to auto-update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE 'plpgsql';