//! JWT Token Generator for Manual Testing
//!
//! This module provides utilities to generate JWT tokens for manual testing
//! of the Secure Service endpoints. Run tests in this module to get valid
//! JWT tokens that can be used with curl or API testing tools.

use common::{Claims, JwtUtils};

/// Default JWT secret for testing (must match secure service config)
pub const TEST_JWT_SECRET: &str = "default_secret";

/// JWT token generator for testing
pub struct TestJwtGenerator {
    pub jwt_utils: JwtUtils,
}

impl TestJwtGenerator {
    /// Create a new JWT generator with test secret
    pub fn new() -> Self {
        Self {
            jwt_utils: JwtUtils::new(TEST_JWT_SECRET.to_string()),
        }
    }

    /// Generate an admin JWT token
    pub fn generate_admin_token(&self, user_id: &str) -> String {
        let claims = Claims::new("admin".to_string(), user_id.to_string(), 3600);
        self.jwt_utils.generate_token(&claims).unwrap()
    }

    /// Generate a user JWT token
    pub fn generate_user_token(&self, user_id: &str) -> String {
        let claims = Claims::new("user".to_string(), user_id.to_string(), 3600);
        self.jwt_utils.generate_token(&claims).unwrap()
    }

    /// Generate a custom JWT token
    pub fn generate_custom_token(&self, role: &str, user_id: &str, expires_in_seconds: i64) -> String {
        let claims = Claims::new(role.to_string(), user_id.to_string(), expires_in_seconds);
        self.jwt_utils.generate_token(&claims).unwrap()
    }

    /// Print tokens for manual testing
    pub fn print_test_tokens(&self) {
        let admin_token = self.generate_admin_token("test_admin_user");
        let user_token = self.generate_user_token("test_regular_user");
        let moderator_token = self.generate_custom_token("moderator", "test_moderator", 7200);

        println!("\n🔐 JWT Tokens for Manual Testing:");
        println!("=====================================");
        println!("\n📋 Admin Token (role: admin, user: test_admin_user):");
        println!("{}\n", admin_token);
        
        println!("📋 User Token (role: user, user: test_regular_user):");
        println!("{}\n", user_token);
        
        println!("📋 Moderator Token (role: moderator, user: test_moderator):");
        println!("{}\n", moderator_token);

        println!("🔧 Usage Examples:");
        println!("=====================================");
        println!("# Test health endpoint (no auth required):");
        println!("curl http://localhost:8082/health\n");
        
        println!("# Test authenticated endpoint with admin token:");
        println!("curl -H \"Authorization: Bearer {}\" \\", admin_token);
        println!("     http://localhost:8082/api/static-datasets\n");
        
        println!("# Test admin-only endpoint (add committee member):");
        println!("curl -X POST \\");
        println!("     -H \"Authorization: Bearer {}\" \\", admin_token);
        println!("     -H \"Content-Type: application/json\" \\");
        println!("     -d '{{\"wallet_address\":\"0x1234567890abcdef1234567890abcdef12345678\",\"is_active\":true}}' \\");
        println!("     http://localhost:8082/api/committee\n");
        
        println!("# Test with user token (should be forbidden for admin endpoints):");
        println!("curl -X POST \\");
        println!("     -H \"Authorization: Bearer {}\" \\", user_token);
        println!("     -H \"Content-Type: application/json\" \\");
        println!("     -d '{{\"wallet_address\":\"0xabcdef1234567890abcdef1234567890abcdef12\",\"is_active\":true}}' \\");
        println!("     http://localhost:8082/api/committee\n");
        
        println!("💡 Tips:");
        println!("- Tokens expire in 1 hour (3600 seconds)");
        println!("- Use admin token for all endpoints including admin-only operations");
        println!("- Use user token to test authorization (should get 403 for admin endpoints)");
        println!("- Health endpoint (/health) doesn't require authentication");
        println!("- All /api/* endpoints require JWT authentication");
    }
}

impl Default for TestJwtGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_generator_admin_token() {
        let generator = TestJwtGenerator::new();
        let token = generator.generate_admin_token("test_admin");
        
        // Validate the token
        let claims = generator.jwt_utils.validate_token(&token).unwrap();
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.sub, "test_admin");
        assert!(!claims.is_expired());
        
        println!("✅ Generated valid admin token: {}", token);
    }

    #[test]
    fn test_jwt_generator_user_token() {
        let generator = TestJwtGenerator::new();
        let token = generator.generate_user_token("test_user");
        
        // Validate the token
        let claims = generator.jwt_utils.validate_token(&token).unwrap();
        assert_eq!(claims.role, "user");
        assert_eq!(claims.sub, "test_user");
        assert!(!claims.is_expired());
        
        println!("✅ Generated valid user token: {}", token);
    }

    #[test]
    fn test_print_all_tokens() {
        let generator = TestJwtGenerator::new();
        generator.print_test_tokens();
        println!("✅ JWT token generation completed");
    }

    /// This test specifically prints tokens for copy-paste usage
    #[test]
    fn test_generate_tokens_for_manual_testing() {
        println!("\n{}", "=".repeat(60));
        println!("🚀 GENERATING JWT TOKENS FOR MANUAL TESTING");
        println!("{}", "=".repeat(60));
        
        let generator = TestJwtGenerator::new();
        generator.print_test_tokens();
        
        println!("{}", "=".repeat(60));
        println!("✅ Copy the tokens above for your API testing!");
        println!("{}", "=".repeat(60));
    }
} 