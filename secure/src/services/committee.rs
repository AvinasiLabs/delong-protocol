use common::{ApiError, ApiResult, PaginationParams, PaginatedResponse};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Committee member database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CommitteeMember {
    pub id: i64,
    pub wallet_address: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request for creating/updating a committee member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCommitteeMemberRequest {
    pub wallet_address: String,
    pub is_active: bool,
}

/// Committee service for database operations
pub struct CommitteeService;

impl CommitteeService {
    /// Create or update a committee member
    pub async fn set_committee_member(
        pool: &PgPool,
        req: SetCommitteeMemberRequest,
    ) -> ApiResult<CommitteeMember> {
        let member = sqlx::query_as!(
            CommitteeMember,
            r#"
            INSERT INTO committee_members (wallet_address, is_active)
            VALUES ($1, $2)
            ON CONFLICT (wallet_address)
            DO UPDATE SET
                is_active = EXCLUDED.is_active,
                updated_at = CURRENT_TIMESTAMP
            RETURNING *
            "#,
            req.wallet_address,
            req.is_active
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to set committee member");
            ApiError::InternalError("Failed to set committee member".to_string())
        })?;

        Ok(member)
    }

    /// Get committee member by ID
    pub async fn get_committee_member_by_id(
        pool: &PgPool,
        id: i64,
    ) -> ApiResult<CommitteeMember> {
        let member = sqlx::query_as!(
            CommitteeMember,
            "SELECT * FROM committee_members WHERE id = $1",
            id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, member_id = %id, "Failed to get committee member");
            ApiError::NotFound("Committee member not found".to_string())
        })?;

        Ok(member)
    }

    /// Get committee member by wallet address
    pub async fn get_committee_member_by_wallet(
        pool: &PgPool,
        wallet_address: &str,
    ) -> ApiResult<Option<CommitteeMember>> {
        let member = sqlx::query_as!(
            CommitteeMember,
            "SELECT * FROM committee_members WHERE wallet_address = $1",
            wallet_address
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, wallet = %wallet_address, "Failed to get committee member by wallet");
            ApiError::InternalError("Failed to get committee member".to_string())
        })?;

        Ok(member)
    }

    /// Check if wallet address is an active committee member
    pub async fn is_active_member(
        pool: &PgPool,
        wallet_address: &str,
    ) -> ApiResult<bool> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM committee_members
            WHERE wallet_address = $1 AND is_active = true
            "#,
            wallet_address
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, wallet = %wallet_address, "Failed to check committee membership");
            ApiError::InternalError("Failed to check membership".to_string())
        })?
        .unwrap_or(0);

        Ok(count > 0)
    }

    /// Get all committee members with pagination
    pub async fn get_committee_members(
        pool: &PgPool,
        params: PaginationParams,
    ) -> ApiResult<PaginatedResponse<CommitteeMember>> {
        let offset = (params.page - 1) * params.limit;

        // Get total count
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM committee_members"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to count committee members");
            ApiError::InternalError("Failed to count members".to_string())
        })?
        .unwrap_or(0);

        // Get paginated results
        let members = sqlx::query_as!(
            CommitteeMember,
            r#"
            SELECT * FROM committee_members
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            params.limit as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get committee members");
            ApiError::InternalError("Failed to get members".to_string())
        })?;

        Ok(PaginatedResponse::new(
            members,
            params.page,
            params.limit,
            total as u64,
        ))
    }

    /// Get all active committee members
    pub async fn get_active_committee_members(
        pool: &PgPool,
    ) -> ApiResult<Vec<CommitteeMember>> {
        let members = sqlx::query_as!(
            CommitteeMember,
            "SELECT * FROM committee_members WHERE is_active = true ORDER BY created_at DESC"
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get active committee members");
            ApiError::InternalError("Failed to get active members".to_string())
        })?;

        Ok(members)
    }

    /// Update committee member status
    pub async fn update_member_status(
        pool: &PgPool,
        wallet_address: &str,
        is_active: bool,
    ) -> ApiResult<CommitteeMember> {
        let member = sqlx::query_as!(
            CommitteeMember,
            r#"
            UPDATE committee_members
            SET is_active = $1,
                updated_at = CURRENT_TIMESTAMP
            WHERE wallet_address = $2
            RETURNING *
            "#,
            is_active,
            wallet_address
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, wallet = %wallet_address, "Failed to update committee member status");
            ApiError::InternalError("Failed to update member status".to_string())
        })?;

        Ok(member)
    }

    /// Get count of active committee members
    pub async fn get_active_member_count(
        pool: &PgPool,
    ) -> ApiResult<i64> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM committee_members WHERE is_active = true"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get active member count");
            ApiError::InternalError("Failed to get member count".to_string())
        })?
        .unwrap_or(0);

        Ok(count)
    }

    /// Get active members with pagination (wrapper for handlers)
    pub async fn get_active_members(
        pool: &PgPool,
        page: i32,
        page_size: i32,
    ) -> ApiResult<Vec<CommitteeMember>> {
        let offset = (page - 1) * page_size;

        let members = sqlx::query_as!(
            CommitteeMember,
            r#"
            SELECT * FROM committee_members
            WHERE is_active = true
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            page_size as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get active committee members");
            ApiError::InternalError("Failed to get active members".to_string())
        })?;

        Ok(members)
    }

    /// Get all members with pagination (wrapper for handlers)
    pub async fn get_all_members(
        pool: &PgPool,
        page: i32,
        page_size: i32,
    ) -> ApiResult<Vec<CommitteeMember>> {
        let offset = (page - 1) * page_size;

        let members = sqlx::query_as!(
            CommitteeMember,
            r#"
            SELECT * FROM committee_members
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            page_size as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get all committee members");
            ApiError::InternalError("Failed to get all members".to_string())
        })?;

        Ok(members)
    }

    /// Get member by wallet (returns error if not found, for handlers)
    pub async fn get_member_by_wallet(
        pool: &PgPool,
        wallet_address: &str,
    ) -> ApiResult<CommitteeMember> {
        let member = sqlx::query_as!(
            CommitteeMember,
            "SELECT * FROM committee_members WHERE wallet_address = $1",
            wallet_address
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, wallet = %wallet_address, "Failed to get committee member by wallet");
            ApiError::NotFound("Committee member not found".to_string())
        })?;

        Ok(member)
    }

    /// Set member (wrapper for set_committee_member)
    pub async fn set_member(
        pool: &PgPool,
        wallet_address: &str,
        is_active: bool,
    ) -> ApiResult<CommitteeMember> {
        let req = SetCommitteeMemberRequest {
            wallet_address: wallet_address.to_string(),
            is_active,
        };
        Self::set_committee_member(pool, req).await
    }
} 