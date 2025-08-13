use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{Create, FindById, Timestamped};
use crate::Result;

/// Contract metadata entity - stores deployed contract addresses
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContractMeta {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub chain_id: String,
    pub deployed_at: DateTime<Utc>,
    pub deployed_by: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ContractMeta {
    /// Get contract address by name and chain_id
    pub async fn get_contract_address(
        pool: &PgPool,
        name: &str,
        chain_id: &str,
    ) -> Result<Option<String>> {
        let result = sqlx::query!(
            "SELECT address FROM contract_meta WHERE name = $1 AND chain_id = $2",
            name,
            chain_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|r| r.address))
    }

    /// Get all contracts ordered by created_at DESC
    pub async fn get_all(pool: &PgPool) -> Result<Vec<Self>> {
        let contracts = sqlx::query_as!(
            ContractMeta,
            r#"
            SELECT id, name, address, chain_id, deployed_at, deployed_by, tx_hash, block_number,
                   metadata as "metadata: serde_json::Value", created_at, updated_at
            FROM contract_meta
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(contracts)
    }

    /// Save a new contract address
    pub async fn save_contract_address(
        pool: &PgPool,
        name: &str,
        address: &str,
        chain_id: &str,
        deployed_by: Option<&str>,
        tx_hash: Option<&str>,
        block_number: Option<i64>,
    ) -> Result<Self> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            INSERT INTO contract_meta (name, address, chain_id, deployed_by, tx_hash, block_number)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, name, address, chain_id, deployed_at, deployed_by, tx_hash, block_number,
                      metadata as "metadata: serde_json::Value", created_at, updated_at
            "#,
            name,
            address,
            chain_id,
            deployed_by,
            tx_hash,
            block_number
        )
        .fetch_one(pool)
        .await?;

        Ok(contract)
    }

    /// Find by name and chain_id
    pub async fn find_by_name_and_chain(
        pool: &PgPool,
        name: &str,
        chain_id: &str,
    ) -> Result<Option<Self>> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"SELECT id, name, address, chain_id, deployed_at, deployed_by, tx_hash, block_number,
                      metadata as "metadata: serde_json::Value", created_at, updated_at
               FROM contract_meta WHERE name = $1 AND chain_id = $2"#,
            name,
            chain_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }

    /// Update contract address
    pub async fn update_address(
        pool: &PgPool,
        name: &str,
        chain_id: &str,
        address: &str,
    ) -> Result<Self> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"
            UPDATE contract_meta
            SET address = $1, updated_at = NOW()
            WHERE name = $2 AND chain_id = $3
            RETURNING id, name, address, chain_id, deployed_at, deployed_by, tx_hash, block_number,
                      metadata as "metadata: serde_json::Value", created_at, updated_at
            "#,
            address,
            name,
            chain_id
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
        &self.updated_at
    }
}

/// Request to create a new contract metadata entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateContractMetaRequest {
    pub name: String,
    pub address: String,
    pub chain_id: String,
    pub deployed_by: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
}

#[async_trait::async_trait]
impl Create for ContractMeta {
    type Request = CreateContractMetaRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        Self::save_contract_address(
            pool,
            &request.name,
            &request.address,
            &request.chain_id,
            request.deployed_by.as_deref(),
            request.tx_hash.as_deref(),
            request.block_number,
        )
        .await
    }
}

#[async_trait::async_trait]
impl FindById for ContractMeta {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let contract = sqlx::query_as!(
            ContractMeta,
            r#"SELECT id, name, address, chain_id, deployed_at, deployed_by, tx_hash, block_number,
                      metadata as "metadata: serde_json::Value", created_at, updated_at
               FROM contract_meta WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }
}
