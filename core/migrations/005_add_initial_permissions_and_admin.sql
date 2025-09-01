-- ============================================
-- Migration: Add initial permissions and admin user
-- ============================================
-- This migration adds:
-- 1. System permissions for all resources
-- 2. Role-permission mappings
-- 3. Default admin user
-- ============================================

-- Insert system permissions
INSERT INTO permissions (name, display_name, description, resource, action) VALUES
-- User management permissions
('user.create', '创建用户', '创建新用户账户', 'user', 'create'),
('user.read', '查看用户', '查看用户信息', 'user', 'read'),
('user.update', '更新用户', '更新用户信息', 'user', 'update'),
('user.delete', '删除用户', '删除用户账户', 'user', 'delete'),

-- Algorithm management permissions
('algorithm.create', '上传算法', '上传新算法', 'algorithm', 'create'),
('algorithm.read', '查看算法', '查看算法详情', 'algorithm', 'read'),
('algorithm.update', '更新算法', '更新算法信息', 'algorithm', 'update'),
('algorithm.delete', '删除算法', '删除算法', 'algorithm', 'delete'),
('algorithm.review', '审核算法', '审核算法提交', 'algorithm', 'review'),

-- Dataset management permissions
('dataset.create', '创建数据集', '创建新数据集', 'dataset', 'create'),
('dataset.read', '查看数据集', '查看数据集', 'dataset', 'read'),
('dataset.update', '更新数据集', '更新数据集', 'dataset', 'update'),
('dataset.delete', '删除数据集', '删除数据集', 'dataset', 'delete'),

-- Report management permissions
('report.create', '生成报告', '生成分析报告', 'report', 'create'),
('report.read', '查看报告', '查看分析报告', 'report', 'read'),
('report.export', '导出报告', '导出报告文件', 'report', 'export'),

-- System management permissions
('system.config', '系统配置', '修改系统配置', 'system', 'config'),
('system.monitor', '系统监控', '监控系统状态', 'system', 'monitor'),
('system.audit', '审计日志', '查看审计日志', 'system', 'audit')
ON CONFLICT (name) DO NOTHING;

-- Assign permissions to roles
-- Scientist permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'scientist' AND p.name IN (
    'algorithm.create',
    'algorithm.read',
    'algorithm.update',
    'dataset.read',
    'report.create',
    'report.read',
    'report.export'
)
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- Committee member permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'committee' AND p.name IN (
    'algorithm.create',
    'algorithm.read',
    'algorithm.update',
    'algorithm.review',
    'dataset.create',
    'dataset.read',
    'dataset.update',
    'dataset.delete',
    'report.read',
    'report.export',
    'user.read'
)
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- Admin permissions (all permissions)
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'admin'
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- Create default admin user
-- Default password: Admin@123456
-- Password hash generated using bcrypt with 12 rounds
-- IMPORTANT: Change this password immediately after first login!
INSERT INTO users (username, email, password_hash, role, status, email_verified, provider) VALUES
('admin', 'admin@delong.com', '$2b$12$818A0.u8S7A8ya4VPDXqweT/YRHY8mWQzBA9xKOroYc2phHCvGD9S', 'admin', 'active', true, 'email')
ON CONFLICT (username) DO NOTHING;

-- Output migration results
DO $$
DECLARE
    admin_exists BOOLEAN;
    perm_count INTEGER;
    role_perm_count INTEGER;
    scientist_perms INTEGER;
    committee_perms INTEGER;
    admin_perms INTEGER;
BEGIN
    -- Check if admin user was created
    SELECT EXISTS(SELECT 1 FROM users WHERE username = 'admin') INTO admin_exists;

    -- Count permissions
    SELECT COUNT(*) FROM permissions INTO perm_count;

    -- Count role permissions
    SELECT COUNT(*) FROM role_permissions INTO role_perm_count;

    -- Count permissions per role
    SELECT COUNT(*) FROM role_permissions rp
    JOIN roles r ON r.id = rp.role_id
    WHERE r.name = 'scientist' INTO scientist_perms;

    SELECT COUNT(*) FROM role_permissions rp
    JOIN roles r ON r.id = rp.role_id
    WHERE r.name = 'committee' INTO committee_perms;

    SELECT COUNT(*) FROM role_permissions rp
    JOIN roles r ON r.id = rp.role_id
    WHERE r.name = 'admin' INTO admin_perms;

    -- Output status
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Initial permissions and admin user migration completed:';
    RAISE NOTICE '  - Total permissions created: %', perm_count;
    RAISE NOTICE '  - Total role-permission mappings: %', role_perm_count;
    RAISE NOTICE '  - Scientist permissions: %', scientist_perms;
    RAISE NOTICE '  - Committee permissions: %', committee_perms;
    RAISE NOTICE '  - Admin permissions: %', admin_perms;
    RAISE NOTICE '  - Admin user exists: %', admin_exists;

    IF admin_exists THEN
        RAISE NOTICE '========================================';
        RAISE NOTICE 'Default admin credentials:';
        RAISE NOTICE '  Username: admin';
        RAISE NOTICE '  Email: admin@delong.com';
        RAISE NOTICE '  Password: Admin@123456';
        RAISE NOTICE '========================================';
        RAISE NOTICE 'SECURITY WARNING: Please change the admin password immediately!';
        RAISE NOTICE '========================================';
    END IF;
END $$;
