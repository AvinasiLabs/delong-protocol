use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// Entity types
pub const ENTITY_TYPE_COMMITTEE: &str = "COMMITTEE";
pub const TX_STATUS_CONFIRMED: &str = "CONFIRMED";

/// Represents a committee member in the database
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct CommitteeMember {
    pub id: i64,
    pub member_wallet: String,
    pub is_approved: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request model for creating/updating committee member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCommitteeMemberRequest {
    pub member_wallet: String,
    pub is_approved: bool,
}

/// Response model for committee member info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitteeMemberInfo {
    pub id: i64,
    pub member_wallet: String,
    pub is_approved: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<CommitteeMember> for CommitteeMemberInfo {
    fn from(member: CommitteeMember) -> Self {
        CommitteeMemberInfo {
            id: member.id,
            member_wallet: member.member_wallet,
            is_approved: member.is_approved,
            created_at: member.created_at,
            updated_at: member.updated_at,
        }
    }
}

impl CommitteeMember {
    /// Create or update a committee member (upsert). Must be used within a transaction.
    pub async fn upsert(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        member_wallet: &str,
        is_approved: bool,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as(
            r#"
            INSERT INTO committee_members (member_wallet, is_approved)
            VALUES ($1, $2)
            ON CONFLICT (member_wallet) 
            DO UPDATE SET is_approved = $2, updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(member_wallet)
        .bind(is_approved)
        .fetch_one(&mut **tx)
        .await
    }

    /// Get confirmed committee members with pagination
    pub async fn get_confirmed_paginated(
        pool: &PgPool,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<Self>, i64), sqlx::Error> {
        let offset = (page - 1) * page_size;

        // Get total count
        let total_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) as count
            FROM committee_members cm
            JOIN blockchain_transactions bt ON bt.entity_id = cm.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_COMMITTEE)
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let members = sqlx::query_as::<_, CommitteeMember>(
            r#"
            SELECT cm.*
            FROM committee_members cm
            JOIN blockchain_transactions bt ON bt.entity_id = cm.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            ORDER BY cm.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_COMMITTEE)
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((members, total_count))
    }

    /// Get confirmed committee member by ID
    pub async fn get_confirmed_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        let member = sqlx::query_as::<_, CommitteeMember>(
            r#"
            SELECT cm.*
            FROM committee_members cm
            JOIN blockchain_transactions bt ON bt.entity_id = cm.id
            WHERE cm.id = $1 AND bt.status = $2 AND bt.entity_type = $3
            "#,
        )
        .bind(id)
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_COMMITTEE)
        .fetch_optional(pool)
        .await?;

        Ok(member)
    }

    /// Get committee member by wallet address
    pub async fn get_by_wallet(pool: &PgPool, wallet: &str) -> Result<Option<Self>, sqlx::Error> {
        let member = sqlx::query_as::<_, CommitteeMember>(
            r#"
            SELECT cm.*
            FROM committee_members cm
            JOIN blockchain_transactions bt ON bt.entity_id = cm.id
            WHERE cm.member_wallet = $1 AND bt.status = $2 AND bt.entity_type = $3
            "#,
        )
        .bind(wallet)
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_COMMITTEE)
        .fetch_optional(pool)
        .await?;

        Ok(member)
    }

    /// Get all confirmed committee members (no pagination)
    pub async fn get_all_confirmed(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        let members = sqlx::query_as::<_, CommitteeMember>(
            r#"
            SELECT cm.*
            FROM committee_members cm
            JOIN blockchain_transactions bt ON bt.entity_id = cm.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            ORDER BY cm.created_at DESC
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_COMMITTEE)
        .fetch_all(pool)
        .await?;

        Ok(members)
    }

    /// Check if a wallet is an approved committee member
    pub async fn is_approved_member(pool: &PgPool, wallet: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT cm.is_approved
            FROM committee_members cm
            JOIN blockchain_transactions bt ON bt.entity_id = cm.id
            WHERE cm.member_wallet = $1 AND bt.status = $2 AND bt.entity_type = $3
            "#,
        )
        .bind(wallet)
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_COMMITTEE)
        .fetch_optional(pool)
        .await?;

        Ok(result.unwrap_or(false))
    }
} 