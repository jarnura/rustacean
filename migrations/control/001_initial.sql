-- Control-plane schema: Wave 1 foundation
-- Enables gen_random_bytes() for token generation (Wave 2 auth)
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
