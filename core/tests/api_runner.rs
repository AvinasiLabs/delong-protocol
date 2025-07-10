
// This file is the main entry point for all API integration tests.
// It includes all other test files as modules.

#![allow(dead_code)]
#![allow(unused_imports)]

mod common;

mod api {
    mod helpers;
    mod health_test;
    mod auth_test;
    mod ai_audit_test;
    // Add other test modules here as they are created
    // mod admin_test;
} 