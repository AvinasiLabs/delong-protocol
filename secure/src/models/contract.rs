use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Contract metadata model for storing deployed contract addresses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractMeta {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub created_at: DateTime<Utc>,
}

/// Request model for saving contract address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveContractRequest {
    pub name: String,
    pub address: String,
}

/// Response model for contract info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInfo {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub created_at: DateTime<Utc>,
}

impl From<ContractMeta> for ContractInfo {
    fn from(contract: ContractMeta) -> Self {
        ContractInfo {
            id: contract.id,
            name: contract.name,
            address: contract.address,
            created_at: contract.created_at,
        }
    }
}

impl ContractMeta {
    /// Get contract address by name
    pub async fn get_address_by_name(pool: &PgPool, name: &str) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT address FROM contract_meta
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }

    /// Get all contracts
    pub async fn get_all(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        let contracts = sqlx::query_as!(
            ContractMeta,
            r#"
            SELECT * FROM contract_meta
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(contracts)
    }

    /// Get contract by ID
    pub async fn get_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            SELECT * FROM contract_meta
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }

    /// Get contract by name
    pub async fn get_by_name(pool: &PgPool, name: &str) -> Result<Option<Self>, sqlx::Error> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            SELECT * FROM contract_meta
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }

    /// Get contract by address
    pub async fn get_by_address(pool: &PgPool, address: &str) -> Result<Option<Self>, sqlx::Error> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            SELECT * FROM contract_meta
            WHERE address = $1
            "#,
            address
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }

    /// Save contract address
    pub async fn save_address(pool: &PgPool, name: &str, address: &str) -> Result<Self, sqlx::Error> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            INSERT INTO contract_meta (name, address)
            VALUES ($1, $2)
            RETURNING *
            "#,
            name,
            address
        )
        .fetch_one(pool)
        .await?;

        Ok(contract)
    }

    /// Update contract address
    pub async fn update_address(
        pool: &PgPool,
        name: &str,
        address: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            UPDATE contract_meta
            SET address = $1
            WHERE name = $2
            RETURNING *
            "#,
            address,
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }

    /// Upsert contract address (insert or update)
    pub async fn upsert_address(
        pool: &PgPool,
        name: &str,
        address: &str,
    ) -> Result<Self, sqlx::Error> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            INSERT INTO contract_meta (name, address)
            VALUES ($1, $2)
            ON CONFLICT (name) 
            DO UPDATE SET address = $2
            RETURNING *
            "#,
            name,
            address
        )
        .fetch_one(pool)
        .await?;

        Ok(contract)
    }

    /// Delete contract
    pub async fn delete(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            DELETE FROM contract_meta
            WHERE name = $1
            "#,
            name
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get contracts with pagination
    pub async fn get_paginated(
        pool: &PgPool,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<Self>, i64), sqlx::Error> {
        let offset = (page - 1) * page_size;

        // Get total count
        let total_count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM contract_meta
            "#
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        // Get paginated results
        let contracts = sqlx::query_as!(
            ContractMeta,
            r#"
            SELECT * FROM contract_meta
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            page_size,
            offset
        )
        .fetch_all(pool)
        .await?;

        Ok((contracts, total_count))
    }

    /// Check if contract exists
    pub async fn exists(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM contract_meta
            WHERE name = $1
            "#,
            name
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        Ok(count > 0)
    }
} 