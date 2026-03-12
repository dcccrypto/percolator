-- Seed admin_users table with project owner
-- The admin dashboard requires a matching row in admin_users for login.
-- Without this, the /admin page redirects to /admin/login after auth check fails.

-- NOTE: The seed.sql file has placeholder emails (admin@percolator.com) that
-- don't match any real Supabase auth user. This migration inserts the actual
-- project owner email.

INSERT INTO admin_users (email)
VALUES ('khubair@dcccrypto.com')
ON CONFLICT (email) DO NOTHING;
