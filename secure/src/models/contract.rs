use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{Create, FindById, Timestamped};
use crate::error::{DbErrorExt, Result};

/// Contract metadata entity - stores deployed contract addresses
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContractMeta {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub created_at: DateTime<Utc>,
}

impl ContractMeta {
    /// Get contract address by name
    pub async fn get_contract_address(pool: &PgPool, name: &str) -> Result<Option<String>> {
        let result = sqlx::query!("SELECT address FROM contract_metas WHERE name = $1", name)
            .fetch_optional(pool)
            .await?;

        Ok(result.map(|r| r.address))
    }

    /// Get all contracts ordered by created_at DESC
    pub async fn get_all(pool: &PgPool) -> Result<Vec<Self>> {
        let contracts = sqlx::query_as!(
            ContractMeta,
            r#"
            SELECT * FROM contract_metas
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(contracts)
    }

    /// Save a new contract address
    pub async fn save_contract_address(pool: &PgPool, name: &str, address: &str) -> Result<Self> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            INSERT INTO contract_metas (name, address)
            VALUES ($1, $2)
            RETURNING *
            "#,
            name,
            address
        )
        .fetch_one(pool)
        .await
        .conflict_msg("Contract with this name already exists")?;

        Ok(contract)
    }

    /// Find by name
    pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<Self>> {
        let contract = sqlx::query_as!(
            ContractMeta,
            "SELECT * FROM contract_metas WHERE name = $1",
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }

    /// Update contract address
    pub async fn update_address(pool: &PgPool, name: &str, address: &str) -> Result<Self> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            UPDATE contract_metas
            SET address = $1
            WHERE name = $2
            RETURNING *
            "#,
            address,
            name
        )
        .fetch_one(pool)
        .await?;

        Ok(contract)
    }
}

impl Timestamped for ContractMeta {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        // ContractMeta doesn't have updated_at, so we return created_at
        &self.created_at
    }
}

/// Request to create a new contract metadata entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateContractMetaRequest {
    pub name: String,
    pub address: String,
}

#[async_trait::async_trait]
impl Create for ContractMeta {
    type Request = CreateContractMetaRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        Self::save_contract_address(pool, &request.name, &request.address).await
    }
}

#[async_trait::async_trait]
impl FindById for ContractMeta {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let contract = sqlx::query_as!(
            ContractMeta,
            "SELECT * FROM contract_metas WHERE id = $1",
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }
}
