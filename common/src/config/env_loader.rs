use std::env;
use std::fmt::Debug;
use std::path::Path;

/// Environment variable loader with service prefix support and fallback files
///
/// This loader provides a unified way to load environment variables with the following priority:
/// 1. System environment variables (highest priority)
/// 2. Service-specific prefix (e.g., GATEWAY_*)
/// 3. Shared prefix (SHARED_*)
/// 4. No prefix (for backward compatibility)
/// 5. Default values (lowest priority)
pub struct EnvLoader {
    /// Service-specific prefix (e.g., "GATEWAY_")
    prefix: String,
    /// List of .env files to load in order (first found wins)
    fallback_files: Vec<String>,
    /// Whether to print debug information during loading
    debug: bool,
}

impl EnvLoader {
    /// Create a new EnvLoader for a specific service
    ///
    /// # Arguments
    /// * `service_name` - The service name (e.g., "gateway", "core", "secure")
    ///
    /// # Example
    /// ```
    /// let loader = EnvLoader::new("gateway");
    /// ```
    pub fn new(service_name: &str) -> Self {
        let prefix = format!("{}_", service_name.to_uppercase());
        let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());

        let fallback_files = vec![
            ".env.local".to_string(),
            format!(".env.{}", environment),
            ".env".to_string(),
        ];

        Self {
            prefix,
            fallback_files,
            debug: env::var("CONFIG_DEBUG")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
        }
    }

    /// Load environment variables from fallback files
    ///
    /// Files are loaded in the order specified in `fallback_files`.
    /// If a file doesn't exist, it's silently skipped.
    ///
    /// # Returns
    /// * `Ok(())` - If loading succeeded (even if no files were found)
    /// * `Err(Box<dyn std::error::Error>)` - If there was an error reading a file
    pub fn load_env_files(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut loaded_files = Vec::new();

        for file in &self.fallback_files {
            if Path::new(file).exists() {
                match dotenvy::from_filename(file) {
                    Ok(_) => {
                        loaded_files.push(file.clone());
                        if self.debug {
                            eprintln!("Loaded env file: {}", file);
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load env file {}: {}", file, e);
                    }
                }
            }
        }

        if self.debug {
            eprintln!(
                "EnvLoader for {} loaded {} files: {:?}",
                self.prefix.trim_end_matches('_'),
                loaded_files.len(),
                loaded_files
            );
        }

        Ok(())
    }

    /// Get a string value from environment variables with prefix fallback
    ///
    /// Priority order:
    /// 1. SERVICE_KEY (e.g., GATEWAY_HOST)
    /// 2. SHARED_KEY (e.g., SHARED_HOST)
    /// 3. KEY (e.g., HOST - for backward compatibility)
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    ///
    /// # Returns
    /// * `Some(String)` - If the variable was found
    /// * `None` - If the variable was not found
    pub fn get_string(&self, key: &str) -> Option<String> {
        // 1. Try with service prefix
        let service_key = format!("{}{}", self.prefix, key);
        if let Ok(value) = env::var(&service_key) {
            if self.debug {
                eprintln!("Found {}={} (from service prefix)", service_key, value);
            }
            return Some(value);
        }

        // 2. Try with SHARED_ prefix
        let shared_key = format!("SHARED_{}", key);
        if let Ok(value) = env::var(&shared_key) {
            if self.debug {
                eprintln!("Found {}={} (from shared prefix)", shared_key, value);
            }
            return Some(value);
        }

        // 3. Try without prefix (for backward compatibility)
        if let Ok(value) = env::var(key) {
            if self.debug {
                eprintln!("Found {}={} (no prefix)", key, value);
            }
            return Some(value);
        }

        if self.debug {
            eprintln!(
                "Not found: {} (tried {}, {}, {})",
                key, service_key, shared_key, key
            );
        }

        None
    }

    /// Get a string value or return a default value
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    /// * `default` - The default value to return if not found
    ///
    /// # Returns
    /// * `String` - The value from environment or the default
    pub fn get_string_or_default(&self, key: &str, default: &str) -> String {
        self.get_string(key).unwrap_or_else(|| default.to_string())
    }

    /// Get a u16 value from environment variables
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    ///
    /// # Returns
    /// * `Ok(Some(u16))` - If the variable was found and parsed successfully
    /// * `Ok(None)` - If the variable was not found
    /// * `Err(std::num::ParseIntError)` - If the variable was found but couldn't be parsed
    pub fn get_u16(&self, key: &str) -> Result<Option<u16>, std::num::ParseIntError> {
        if let Some(value) = self.get_string(key) {
            Ok(Some(value.parse()?))
        } else {
            Ok(None)
        }
    }

    /// Get a u16 value or return a default value
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    /// * `default` - The default value to return if not found or parsing failed
    ///
    /// # Returns
    /// * `u16` - The value from environment or the default
    pub fn get_u16_or_default(&self, key: &str, default: u16) -> u16 {
        self.get_u16(key).unwrap_or(None).unwrap_or(default)
    }

    /// Get a u32 value from environment variables
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    ///
    /// # Returns
    /// * `Ok(Some(u32))` - If the variable was found and parsed successfully
    /// * `Ok(None)` - If the variable was not found
    /// * `Err(std::num::ParseIntError)` - If the variable was found but couldn't be parsed
    pub fn get_u32(&self, key: &str) -> Result<Option<u32>, std::num::ParseIntError> {
        if let Some(value) = self.get_string(key) {
            Ok(Some(value.parse()?))
        } else {
            Ok(None)
        }
    }

    /// Get a u32 value or return a default value
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    /// * `default` - The default value to return if not found or parsing failed
    ///
    /// # Returns
    /// * `u32` - The value from environment or the default
    pub fn get_u32_or_default(&self, key: &str, default: u32) -> u32 {
        self.get_u32(key).unwrap_or(None).unwrap_or(default)
    }

    /// Get a u64 value from environment variables
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    ///
    /// # Returns
    /// * `Ok(Some(u64))` - If the variable was found and parsed successfully
    /// * `Ok(None)` - If the variable was not found
    /// * `Err(std::num::ParseIntError)` - If the variable was found but couldn't be parsed
    pub fn get_u64(&self, key: &str) -> Result<Option<u64>, std::num::ParseIntError> {
        if let Some(value) = self.get_string(key) {
            Ok(Some(value.parse()?))
        } else {
            Ok(None)
        }
    }

    /// Get a u64 value or return a default value
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    /// * `default` - The default value to return if not found or parsing failed
    ///
    /// # Returns
    /// * `u64` - The value from environment or the default
    pub fn get_u64_or_default(&self, key: &str, default: u64) -> u64 {
        self.get_u64(key).unwrap_or(None).unwrap_or(default)
    }

    /// Get a boolean value from environment variables
    ///
    /// Recognizes the following as true (case-insensitive):
    /// - "true", "1", "yes", "on", "enabled"
    ///
    /// Everything else is considered false.
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    ///
    /// # Returns
    /// * `Some(bool)` - If the variable was found
    /// * `None` - If the variable was not found
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_string(key).map(|v| {
            matches!(
                v.to_lowercase().as_str(),
                "true" | "1" | "yes" | "on" | "enabled"
            )
        })
    }

    /// Get a boolean value or return a default value
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    /// * `default` - The default value to return if not found
    ///
    /// # Returns
    /// * `bool` - The value from environment or the default
    pub fn get_bool_or_default(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }

    /// Get a floating-point value from environment variables
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    ///
    /// # Returns
    /// * `Ok(Some(f64))` - If the variable was found and parsed successfully
    /// * `Ok(None)` - If the variable was not found
    /// * `Err(std::num::ParseFloatError)` - If the variable was found but couldn't be parsed
    pub fn get_f64(&self, key: &str) -> Result<Option<f64>, std::num::ParseFloatError> {
        if let Some(value) = self.get_string(key) {
            Ok(Some(value.parse()?))
        } else {
            Ok(None)
        }
    }

    /// Get a floating-point value or return a default value
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    /// * `default` - The default value to return if not found or parsing failed
    ///
    /// # Returns
    /// * `f64` - The value from environment or the default
    pub fn get_f64_or_default(&self, key: &str, default: f64) -> f64 {
        self.get_f64(key).unwrap_or(None).unwrap_or(default)
    }

    /// Get a comma-separated list of strings from environment variables
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    ///
    /// # Returns
    /// * `Option<Vec<String>>` - If the variable was found, returns a vector of trimmed strings
    /// * `None` - If the variable was not found
    pub fn get_string_list(&self, key: &str) -> Option<Vec<String>> {
        self.get_string(key).map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
    }

    /// Get a comma-separated list of strings or return a default value
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    /// * `default` - The default value to return if not found
    ///
    /// # Returns
    /// * `Vec<String>` - The value from environment or the default
    pub fn get_string_list_or_default(&self, key: &str, default: Vec<String>) -> Vec<String> {
        self.get_string_list(key).unwrap_or(default)
    }

    /// Check if a key exists in the environment (useful for optional features)
    ///
    /// # Arguments
    /// * `key` - The environment variable key (without prefix)
    ///
    /// # Returns
    /// * `bool` - True if the key exists, false otherwise
    pub fn has_key(&self, key: &str) -> bool {
        self.get_string(key).is_some()
    }

    /// Get the service prefix used by this loader
    ///
    /// # Returns
    /// * `&str` - The service prefix (e.g., "GATEWAY_")
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Enable or disable debug output
    ///
    /// # Arguments
    /// * `debug` - Whether to enable debug output
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }
}

impl Debug for EnvLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvLoader")
            .field("prefix", &self.prefix)
            .field("fallback_files", &self.fallback_files)
            .field("debug", &self.debug)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    fn test_new_loader() {
        let loader = EnvLoader::new("gateway");
        assert_eq!(loader.prefix(), "GATEWAY_");
        assert!(loader.fallback_files.contains(&".env".to_string()));
        assert!(loader.fallback_files.contains(&".env.local".to_string()));
    }

    #[test]
    #[serial]
    fn test_prefix_priority() {
        unsafe {
            env::set_var("GATEWAY_TEST_KEY", "gateway_value");
            env::set_var("SHARED_TEST_KEY", "shared_value");
            env::set_var("TEST_KEY", "plain_value");
        }

        let loader = EnvLoader::new("gateway");

        // Should prefer service prefix
        assert_eq!(
            loader.get_string("TEST_KEY"),
            Some("gateway_value".to_string())
        );

        // Remove service prefix, should use shared
        unsafe {
            env::remove_var("GATEWAY_TEST_KEY");
        }
        assert_eq!(
            loader.get_string("TEST_KEY"),
            Some("shared_value".to_string())
        );

        // Remove shared prefix, should use plain
        unsafe {
            env::remove_var("SHARED_TEST_KEY");
        }
        assert_eq!(
            loader.get_string("TEST_KEY"),
            Some("plain_value".to_string())
        );

        // Clean up
        unsafe {
            env::remove_var("TEST_KEY");
        }
    }

    #[test]
    #[serial]
    fn test_type_conversions() {
        unsafe {
            env::set_var("TEST_PORT", "8080");
            env::set_var("TEST_ENABLED", "true");
            env::set_var("TEST_TIMEOUT", "30.5");
            env::set_var("TEST_LIST", "a,b,c");
        }

        let loader = EnvLoader::new("test");

        assert_eq!(loader.get_u16("PORT"), Ok(Some(8080)));
        assert_eq!(loader.get_bool("ENABLED"), Some(true));
        assert_eq!(loader.get_f64("TIMEOUT"), Ok(Some(30.5)));
        assert_eq!(
            loader.get_string_list("LIST"),
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );

        // Clean up
        unsafe {
            env::remove_var("TEST_PORT");
            env::remove_var("TEST_ENABLED");
            env::remove_var("TEST_TIMEOUT");
            env::remove_var("TEST_LIST");
        }
    }

    #[test]
    fn test_defaults() {
        let loader = EnvLoader::new("test");

        assert_eq!(
            loader.get_string_or_default("NONEXISTENT", "default"),
            "default"
        );
        assert_eq!(loader.get_u16_or_default("NONEXISTENT", 8080), 8080);
        assert_eq!(loader.get_bool_or_default("NONEXISTENT", true), true);
        assert_eq!(loader.get_f64_or_default("NONEXISTENT", 1.5), 1.5);
    }

    #[test]
    #[serial]
    fn test_boolean_parsing() {
        let loader = EnvLoader::new("test");

        for true_val in &[
            "true", "TRUE", "1", "yes", "YES", "on", "ON", "enabled", "ENABLED",
        ] {
            unsafe {
                env::set_var("TEST_BOOL", true_val);
            }
            assert_eq!(
                loader.get_bool("BOOL"),
                Some(true),
                "Failed for value: {}",
                true_val
            );
        }

        for false_val in &[
            "false", "FALSE", "0", "no", "NO", "off", "OFF", "disabled", "DISABLED", "random",
        ] {
            unsafe {
                env::set_var("TEST_BOOL", false_val);
            }
            assert_eq!(
                loader.get_bool("BOOL"),
                Some(false),
                "Failed for value: {}",
                false_val
            );
        }

        unsafe {
            env::remove_var("TEST_BOOL");
        }
    }

    #[test]
    #[serial]
    fn test_has_key() {
        let loader = EnvLoader::new("test");

        unsafe {
            env::set_var("TEST_EXISTS", "value");
        }
        assert!(loader.has_key("EXISTS"));

        unsafe {
            env::remove_var("TEST_EXISTS");
        }
        assert!(!loader.has_key("EXISTS"));
    }
}
