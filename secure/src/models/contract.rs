use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{Create, FindById, Timestamped};
use crate::Result;

/// Contract metadata entity - stores deployed contract addresses
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Contract {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub chain_id: i64,
    pub abi: Option<String>,
    pub deployed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Contract {
    /// Get contract address by name and chain_id
    pub async fn get_contract_address(
        pool: &PgPool,
        name: &str,
        chain_id: i64,
    ) -> Result<Option<String>> {
        let result = sqlx::query!(
            "SELECT address FROM contract WHERE name = $1 AND chain_id = $2",
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
            Contract,
            r#"
            SELECT id, name, address, chain_id, abi, deployed_at, created_at, updated_at
            FROM contract
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
        chain_id: i64,
    ) -> Result<Self> {
        let contract = sqlx::query_as!(
            Contract,
            r#"
            INSERT INTO contract (name, address, chain_id)
            VALUES ($1, $2, $3)
            RETURNING id, name, address, chain_id, abi, deployed_at, created_at, updated_at
            "#,
            name,
            address,
            chain_id
        )
        .fetch_one(pool)
        .await?;

        Ok(contract)
    }

    /// Find by name and chain_id
    pub async fn find_by_name_and_chain(
        pool: &PgPool,
        name: &str,
        chain_id: i64,
    ) -> Result<Option<Self>> {
        let contract = sqlx::query_as!(
            Contract,
            r#"SELECT id, name, address, chain_id, deployed_at, abi, created_at, updated_at
               FROM contract WHERE name = $1 AND chain_id = $2"#,
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
        chain_id: i64,
        address: &str,
    ) -> Result<Self> {
        let contract = sqlx::query_as!(
            Contract,
            r#"
            UPDATE contract
            SET address = $1, updated_at = NOW()
            WHERE name = $2 AND chain_id = $3
            RETURNING id, name, address, chain_id, deployed_at, abi, created_at, updated_at
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

impl Timestamped for Contract {
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
    pub chain_id: i64,
    pub abi: Option<String>,
}

#[async_trait::async_trait]
impl Create for Contract {
    type Request = CreateContractMetaRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        Self::save_contract_address(pool, &request.name, &request.address, request.chain_id).await
    }
}

#[async_trait::async_trait]
impl FindById for Contract {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let contract = sqlx::query_as!(
            Contract,
            r#"SELECT id, name, address, chain_id, deployed_at, created_at, updated_at, abi
               FROM contract WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(contract)
    }
}
