-- ============================================
-- Migration: Fix admin password hash
-- ============================================
-- This migration updates the admin password hash to ensure
-- the default password "Admin@123456" works correctly
-- ============================================

-- Update admin password hash
-- Password: Admin@123456
-- Hash generated using bcrypt with cost factor 12
UPDATE users
SET password_hash = '$2b$12$q1Mp93fMBjVDmnhS6D3.j.hMc9da0Fd1.fkBd8DNdj0EKFk/7Dt52',
    updated_at = CURRENT_TIMESTAMP
WHERE username = 'admin' AND role = 'admin';

-- Verify the update
DO $$
DECLARE
    admin_updated INTEGER;
BEGIN
    -- Check how many admin users were updated
    SELECT COUNT(*)
    FROM users
    WHERE username = 'admin'
      AND role = 'admin'
      AND password_hash = '$2b$12$q1Mp93fMBjVDmnhS6D3.j.hMc9da0Fd1.fkBd8DNdj0EKFk/7Dt52'
    INTO admin_updated;

    IF admin_updated > 0 THEN
        RAISE NOTICE '========================================';
        RAISE NOTICE '✅ Admin password hash updated successfully';
        RAISE NOTICE '========================================';
        RAISE NOTICE 'Default admin credentials:';
        RAISE NOTICE '  Email: admin@delong.com';
        RAISE NOTICE '  Password: Admin@123456';
        RAISE NOTICE '========================================';
        RAISE NOTICE '⚠️  SECURITY WARNING: Please change the admin password immediately after first login!';
        RAISE NOTICE '========================================';
    ELSE
        RAISE WARNING 'Admin user password was not updated. Admin user may not exist.';
    END IF;
END $$;
