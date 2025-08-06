use avinapi::{query::PaginationQuery, transport::response::PaginatedData};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{
    blockchain_transaction::{EntityType, TransactionStatus},
    Create, FindById, Timestamped,
};
use crate::error::{DbErrorExt, Result};

/// Committee member entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommitteeMember {
    pub id: i32,
    pub member_wallet: String,
    pub is_approved: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CommitteeMember {
    /// Upsert a committee member (create or update based on wallet)
    pub async fn upsert(pool: &PgPool, member_wallet: &str, is_approved: bool) -> Result<Self> {
        let member = sqlx::query_as!(
            CommitteeMember,
            r#"
            INSERT INTO committee_members (member_wallet, is_approved)
            VALUES ($1, $2)
            ON CONFLICT (member_wallet)
            DO UPDATE SET
                is_approved = EXCLUDED.is_approved,
                updated_at = NOW()
            RETURNING *
            "#,
            member_wallet,
            is_approved
        )
        .fetch_one(pool)
        .await
        .conflict_msg("Failed to upsert committee member")?;

        Ok(member)
    }

    /// Get confirmed committee members with pagination
    pub async fn get_confirmed_members(
        pool: &PgPool,
        pagination: PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM committee_members
            JOIN blockchain_transactions bt ON bt.entity_id = committee_members.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Committee.as_str()
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let members = sqlx::query_as!(
            CommitteeMember,
            r#"
            SELECT committee_members.*
            FROM committee_members
            JOIN blockchain_transactions bt ON bt.entity_id = committee_members.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            ORDER BY committee_members.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Committee.as_str(),
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: members,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Get a confirmed committee member by ID
    pub async fn get_confirmed_by_id(pool: &PgPool, id: i32) -> Result<Option<Self>> {
        let member = sqlx::query_as!(
            CommitteeMember,
            r#"
            SELECT committee_members.*
            FROM committee_members
            JOIN blockchain_transactions bt ON bt.entity_id = committee_members.id
            WHERE bt.status = $1 AND bt.entity_type = $2 AND committee_members.id = $3
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Committee.as_str(),
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(member)
    }

    /// Get a committee member by wallet address
    pub async fn get_by_wallet(pool: &PgPool, wallet: &str) -> Result<Option<Self>> {
        let member = sqlx::query_as!(
            CommitteeMember,
            r#"
            SELECT committee_members.*
            FROM committee_members
            JOIN blockchain_transactions bt ON bt.entity_id = committee_members.id
            WHERE bt.status = $1 AND bt.entity_type = $2 AND committee_members.member_wallet = $3
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Committee.as_str(),
            wallet
        )
        .fetch_optional(pool)
        .await?;

        Ok(member)
    }

    /// Check if a wallet is a committee member
    pub async fn is_committee_member(pool: &PgPool, wallet: &str) -> Result<bool> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM committee_members
                JOIN blockchain_transactions bt ON bt.entity_id = committee_members.id
                WHERE bt.status = $1 AND bt.entity_type = $2 AND committee_members.member_wallet = $3
            )
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Committee.as_str(),
            wallet
        )
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }

    /// Get all approved committee members (regardless of blockchain confirmation)
    pub async fn get_all_approved(pool: &PgPool) -> Result<Vec<Self>> {
        let members = sqlx::query_as!(
            CommitteeMember,
            r#"
            SELECT * FROM committee_members
            WHERE is_approved = true
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(members)
    }
}

impl Timestamped for CommitteeMember {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

/// Request to create a new committee member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommitteeMemberRequest {
    pub member_wallet: String,
    pub is_approved: bool,
}

#[async_trait::async_trait]
impl Create for CommitteeMember {
    type Request = CreateCommitteeMemberRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let member = sqlx::query_as!(
            CommitteeMember,
            r#"
            INSERT INTO committee_members (member_wallet, is_approved)
            VALUES ($1, $2)
            RETURNING *
            "#,
            &request.member_wallet,
            request.is_approved
        )
        .fetch_one(pool)
        .await
        .conflict_msg("Committee member with this wallet already exists")?;

        Ok(member)
    }
}

#[async_trait::async_trait]
impl FindById for CommitteeMember {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        // Convert i64 to i32 for committee member IDs
        let id = id as i32;

        let member = sqlx::query_as!(
            CommitteeMember,
            "SELECT * FROM committee_members WHERE id = $1",
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(member)
    }
}
