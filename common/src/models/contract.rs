//! Contract metadata data models
//!
//! This module contains all data structures related to smart contract metadata,
//! including contract information, deployment data, and contract operations.

use serde::{Deserialize, Serialize};

/// Contract metadata data model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractData {
    /// Unique identifier for the contract
    pub id: u64,
    /// Human-readable name of the contract
    pub name: String,
    /// Contract address on blockchain
    pub address: String,
    /// Creation timestamp in RFC3339 format
    pub created_at: String,
}

/// Request for creating a new contract record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateContractRequest {
    /// Human-readable name of the contract
    pub name: String,
    /// Contract address on blockchain
    pub address: String,
    /// Optional description of the contract
    pub description: Option<String>,
    /// Contract ABI (Application Binary Interface) as JSON string
    pub abi: Option<String>,
    /// Bytecode of the deployed contract
    pub bytecode: Option<String>,
}

/// Request for updating contract metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateContractRequest {
    /// Updated name of the contract
    pub name: Option<String>,
    /// Updated description of the contract
    pub description: Option<String>,
    /// Updated ABI as JSON string
    pub abi: Option<String>,
}

/// Response for contract operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractResponse {
    /// ID of the created or updated contract
    pub id: u64,
    /// Contract address
    pub address: String,
}

/// Extended contract data with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtendedContractData {
    /// Basic contract information
    #[serde(flatten)]
    pub contract: ContractData,
    /// Optional description of the contract
    pub description: Option<String>,
    /// Contract ABI (Application Binary Interface) as JSON string
    pub abi: Option<String>,
    /// Bytecode of the deployed contract
    pub bytecode: Option<String>,
    /// Last update timestamp in RFC3339 format
    pub updated_at: String,
    /// Whether the contract is active
    pub is_active: bool,
}

impl ContractData {
    /// Create a new contract data instance
    pub fn new(id: u64, name: String, address: String, created_at: String) -> Self {
        Self {
            id,
            name,
            address,
            created_at,
        }
    }

    /// Validate the contract address format
    pub fn validate_address(&self) -> Result<(), String> {
        validate_ethereum_address(&self.address)
    }

    /// Check if the contract name is valid
    pub fn validate_name(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Contract name cannot be empty".to_string());
        }

        if self.name.len() > 100 {
            return Err("Contract name cannot exceed 100 characters".to_string());
        }

        Ok(())
    }

    /// Get a short display name for the contract
    pub fn display_name(&self) -> String {
        if self.name.len() > 30 {
            format!("{}...", &self.name[..27])
        } else {
            self.name.clone()
        }
    }
}

impl CreateContractRequest {
    /// Create a new contract creation request
    pub fn new(name: String, address: String) -> Self {
        Self {
            name,
            address,
            description: None,
            abi: None,
            bytecode: None,
        }
    }

    /// Create a new contract creation request with all fields
    pub fn new_with_metadata(
        name: String,
        address: String,
        description: Option<String>,
        abi: Option<String>,
        bytecode: Option<String>,
    ) -> Self {
        Self {
            name,
            address,
            description,
            abi,
            bytecode,
        }
    }

    /// Validate the contract creation request
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Contract name cannot be empty".to_string());
        }

        if self.name.len() > 100 {
            return Err("Contract name cannot exceed 100 characters".to_string());
        }

        validate_ethereum_address(&self.address)?;

        if let Some(desc) = &self.description {
            if desc.len() > 1000 {
                return Err("Contract description cannot exceed 1000 characters".to_string());
            }
        }

        // Validate ABI if provided
        if let Some(abi) = &self.abi {
            if !abi.trim().is_empty() {
                // Basic JSON validation
                if !abi.trim_start().starts_with('[') || !abi.trim_end().ends_with(']') {
                    return Err("Contract ABI must be a valid JSON array".to_string());
                }
            }
        }

        Ok(())
    }
}

impl UpdateContractRequest {
    /// Create a new contract update request
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            abi: None,
        }
    }

    /// Set the name to update
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set the description to update
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the ABI to update
    pub fn with_abi(mut self, abi: String) -> Self {
        self.abi = Some(abi);
        self
    }

    /// Validate the contract update request
    pub fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.name {
            if name.trim().is_empty() {
                return Err("Contract name cannot be empty".to_string());
            }
            if name.len() > 100 {
                return Err("Contract name cannot exceed 100 characters".to_string());
            }
        }

        if let Some(desc) = &self.description {
            if desc.len() > 1000 {
                return Err("Contract description cannot exceed 1000 characters".to_string());
            }
        }

        if let Some(abi) = &self.abi {
            if !abi.trim().is_empty() {
                // Basic JSON validation
                if !abi.trim_start().starts_with('[') || !abi.trim_end().ends_with(']') {
                    return Err("Contract ABI must be a valid JSON array".to_string());
                }
            }
        }

        Ok(())
    }

    /// Check if the request has any updates
    pub fn has_updates(&self) -> bool {
        self.name.is_some() || self.description.is_some() || self.abi.is_some()
    }
}

impl ContractResponse {
    /// Create a new contract response
    pub fn new(id: u64, address: String) -> Self {
        Self { id, address }
    }
}

impl ExtendedContractData {
    /// Create a new extended contract data instance
    pub fn new(
        contract: ContractData,
        description: Option<String>,
        abi: Option<String>,
        bytecode: Option<String>,
        updated_at: String,
        is_active: bool,
    ) -> Self {
        Self {
            contract,
            description,
            abi,
            bytecode,
            updated_at,
            is_active,
        }
    }

    /// Check if the contract is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Get the contract address
    pub fn address(&self) -> &str {
        &self.contract.address
    }

    /// Get the contract name
    pub fn name(&self) -> &str {
        &self.contract.name
    }
}

impl Default for UpdateContractRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate Ethereum address format
fn validate_ethereum_address(address: &str) -> Result<(), String> {
    if address.is_empty() {
        return Err("Contract address cannot be empty".to_string());
    }

    // Check if it starts with 0x
    if !address.starts_with("0x") {
        return Err("Contract address must start with '0x'".to_string());
    }

    // Check if it has the correct length (42 characters total: 2 for "0x" + 40 hex characters)
    if address.len() != 42 {
        return Err("Contract address must be 42 characters long".to_string());
    }

    // Check if all characters after "0x" are valid hex characters
    let hex_part = &address[2..];
    if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Contract address contains invalid hexadecimal characters".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_data_creation() {
        let contract = ContractData::new(
            1,
            "DeLong Core Contract".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(contract.id, 1);
        assert_eq!(contract.name, "DeLong Core Contract");
        assert_eq!(
            contract.address,
            "0x1234567890abcdef1234567890abcdef12345678"
        );
        assert_eq!(contract.created_at, "2023-01-01T00:00:00Z");
    }

    #[test]
    fn test_contract_data_validation() {
        let contract = ContractData::new(
            1,
            "Valid Contract".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert!(contract.validate_address().is_ok());
        assert!(contract.validate_name().is_ok());
    }

    #[test]
    fn test_contract_data_invalid_address() {
        let contract = ContractData::new(
            1,
            "Valid Contract".to_string(),
            "invalid_address".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert!(contract.validate_address().is_err());
    }

    #[test]
    fn test_contract_data_invalid_name() {
        let contract = ContractData::new(
            1,
            "".to_string(), // Empty name
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert!(contract.validate_name().is_err());
    }

    #[test]
    fn test_contract_data_display_name() {
        let short_name_contract = ContractData::new(
            1,
            "Short".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(short_name_contract.display_name(), "Short");

        let long_name_contract = ContractData::new(
            1,
            "This is a very long contract name that exceeds thirty characters".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(
            long_name_contract.display_name(),
            "This is a very long contrac..."
        );
    }

    #[test]
    fn test_create_contract_request() {
        let request = CreateContractRequest::new(
            "Test Contract".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        );

        assert_eq!(request.name, "Test Contract");
        assert_eq!(
            request.address,
            "0x1234567890abcdef1234567890abcdef12345678"
        );
        assert!(request.description.is_none());
        assert!(request.abi.is_none());
        assert!(request.bytecode.is_none());
    }

    #[test]
    fn test_create_contract_request_with_metadata() {
        let request = CreateContractRequest::new_with_metadata(
            "Test Contract".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            Some("A test contract".to_string()),
            Some("[]".to_string()),
            Some("0x608060405234801561001057600080fd5b50".to_string()),
        );

        assert_eq!(request.name, "Test Contract");
        assert_eq!(request.description, Some("A test contract".to_string()));
        assert_eq!(request.abi, Some("[]".to_string()));
        assert!(request.bytecode.is_some());
    }

    #[test]
    fn test_create_contract_request_validation() {
        // Valid request
        let valid_request = CreateContractRequest::new(
            "Valid Contract".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        );
        assert!(valid_request.validate().is_ok());

        // Invalid name (empty)
        let empty_name = CreateContractRequest::new(
            "".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        );
        assert!(empty_name.validate().is_err());

        // Invalid address
        let invalid_address =
            CreateContractRequest::new("Valid Contract".to_string(), "invalid_address".to_string());
        assert!(invalid_address.validate().is_err());
    }

    #[test]
    fn test_update_contract_request() {
        let request = UpdateContractRequest::new()
            .with_name("Updated Name".to_string())
            .with_description("Updated description".to_string())
            .with_abi("[]".to_string());

        assert_eq!(request.name, Some("Updated Name".to_string()));
        assert_eq!(request.description, Some("Updated description".to_string()));
        assert_eq!(request.abi, Some("[]".to_string()));
        assert!(request.has_updates());
    }

    #[test]
    fn test_update_contract_request_no_updates() {
        let request = UpdateContractRequest::new();
        assert!(!request.has_updates());
    }

    #[test]
    fn test_contract_response() {
        let response =
            ContractResponse::new(42, "0x1234567890abcdef1234567890abcdef12345678".to_string());

        assert_eq!(response.id, 42);
        assert_eq!(
            response.address,
            "0x1234567890abcdef1234567890abcdef12345678"
        );
    }

    #[test]
    fn test_validate_ethereum_address() {
        // Valid address
        assert!(validate_ethereum_address("0x1234567890abcdef1234567890abcdef12345678").is_ok());

        // Empty address
        assert!(validate_ethereum_address("").is_err());

        // No 0x prefix
        assert!(validate_ethereum_address("1234567890abcdef1234567890abcdef12345678").is_err());

        // Wrong length
        assert!(validate_ethereum_address("0x12345").is_err());

        // Invalid hex characters
        assert!(validate_ethereum_address("0x1234567890abcdef1234567890abcdef1234567g").is_err());
    }

    #[test]
    fn test_serialization_deserialization() {
        let contract = ContractData::new(
            1,
            "Test Contract".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        let json = serde_json::to_string(&contract).unwrap();
        let deserialized: ContractData = serde_json::from_str(&json).unwrap();
        assert_eq!(contract, deserialized);

        let request = CreateContractRequest::new(
            "Test Contract".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        );

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: CreateContractRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }
}
