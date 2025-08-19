//! Validation utilities and regex patterns
//!
//! This module provides common validation patterns and utilities
//! used throughout the application.

use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern for validating usernames
/// Allows letters, numbers, and underscores, 3-20 characters
pub const USERNAME_PATTERN: &str = r"^[a-zA-Z0-9_]{3,20}$";
pub static USERNAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(USERNAME_PATTERN).expect("Invalid username regex pattern"));

/// Regex pattern for validating wallet addresses (Ethereum format)
/// Checks for 0x prefix followed by 40 hexadecimal characters
pub const WALLET_ADDRESS_PATTERN: &str = r"^0x[a-fA-F0-9]{40}$";
pub static WALLET_ADDRESS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(WALLET_ADDRESS_PATTERN).expect("Invalid wallet address regex pattern")
});

/// Regex pattern for validating phone numbers
/// Supports international format with optional country code
pub const PHONE_PATTERN: &str = r"^\+?[1-9]\d{1,14}$";
pub static PHONE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(PHONE_PATTERN).expect("Invalid phone regex pattern"));

/// Regex pattern for validating URLs
/// Basic URL validation pattern
pub const URL_PATTERN: &str = r"^https?://[^\s/$.?#].[^\s]*$";
pub static URL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(URL_PATTERN).expect("Invalid URL regex pattern"));

/// Regex pattern for validating slugs
/// URL-friendly strings with lowercase letters, numbers, and hyphens
pub const SLUG_PATTERN: &str = r"^[a-z0-9]+(?:-[a-z0-9]+)*$";
pub static SLUG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(SLUG_PATTERN).expect("Invalid slug regex pattern"));

/// Regex pattern for validating API keys
/// Alphanumeric characters with specific length
pub const API_KEY_PATTERN: &str = r"^[A-Za-z0-9]{32,64}$";
pub static API_KEY_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(API_KEY_PATTERN).expect("Invalid API key regex pattern"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_username_regex() {
        assert!(USERNAME_REGEX.is_match("user123"));
        assert!(USERNAME_REGEX.is_match("test_user"));
        assert!(USERNAME_REGEX.is_match("User_123"));
        assert!(!USERNAME_REGEX.is_match("us")); // Too short
        assert!(!USERNAME_REGEX.is_match("user-name")); // Contains hyphen
        assert!(!USERNAME_REGEX.is_match("user name")); // Contains space
        assert!(!USERNAME_REGEX.is_match("user@123")); // Contains @
    }

    #[test]
    fn test_wallet_address_regex() {
        assert!(WALLET_ADDRESS_REGEX.is_match("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7"));
        assert!(!WALLET_ADDRESS_REGEX.is_match("742d35Cc6634C0532925a3b844Bc9e7595f0bEb7")); // Missing 0x
        assert!(!WALLET_ADDRESS_REGEX.is_match("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb")); // Too short
        assert!(!WALLET_ADDRESS_REGEX.is_match("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7Z")); // Invalid char
    }

    #[test]
    fn test_phone_regex() {
        assert!(PHONE_REGEX.is_match("+14155552671"));
        assert!(PHONE_REGEX.is_match("14155552671"));
        assert!(PHONE_REGEX.is_match("+861234567890"));
        assert!(!PHONE_REGEX.is_match("0123456789")); // Starts with 0
        assert!(!PHONE_REGEX.is_match("123-456-7890")); // Contains hyphens
    }

    #[test]
    fn test_url_regex() {
        assert!(URL_REGEX.is_match("https://example.com"));
        assert!(URL_REGEX.is_match("http://example.com/path"));
        assert!(URL_REGEX.is_match("https://sub.example.com/path?query=value"));
        assert!(!URL_REGEX.is_match("ftp://example.com")); // Not http/https
        assert!(!URL_REGEX.is_match("example.com")); // Missing protocol
    }

    #[test]
    fn test_slug_regex() {
        assert!(SLUG_REGEX.is_match("my-awesome-post"));
        assert!(SLUG_REGEX.is_match("article123"));
        assert!(SLUG_REGEX.is_match("test"));
        assert!(!SLUG_REGEX.is_match("My-Post")); // Contains uppercase
        assert!(!SLUG_REGEX.is_match("my_post")); // Contains underscore
        assert!(!SLUG_REGEX.is_match("my--post")); // Double hyphen
    }
}
