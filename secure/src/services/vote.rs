use common::{ApiError, ApiResult, PaginationParams, PaginatedResponse};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Vote database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Vote {
    pub id: i64,
    pub execution_id: i64,
    pub voter_wallet: String,
    pub decision: String, // "APPROVE" or "REJECT"
    pub voted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request for casting a vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastVoteRequest {
    pub execution_id: i64,
    pub voter_wallet: String,
    pub decision: String,
}

/// Vote tally result
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VoteTally {
    pub execution_id: i64,
    pub total_votes: i64,
    pub approve_votes: i64,
    pub reject_votes: i64,
    pub total_committee_members: i64,
    pub is_complete: bool,
    pub is_approved: bool,
}

/// Vote service for database operations
pub struct VoteService;

impl VoteService {
    /// Cast a vote for an algorithm execution
    pub async fn cast_vote(
        pool: &PgPool,
        req: CastVoteRequest,
    ) -> ApiResult<Vote> {
        // Check if voter has already voted
        let existing_vote = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM votes WHERE execution_id = $1 AND voter_wallet = $2",
            req.execution_id,
            req.voter_wallet
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to check existing vote");
            ApiError::InternalError("Failed to check vote".to_string())
        })?
        .unwrap_or(0);

        if existing_vote > 0 {
            return Err(ApiError::BadRequest("Voter has already voted for this execution".to_string()));
        }

        // Validate decision
        if req.decision != "APPROVE" && req.decision != "REJECT" {
            return Err(ApiError::BadRequest("Decision must be APPROVE or REJECT".to_string()));
        }

        let vote = sqlx::query_as!(
            Vote,
            r#"
            INSERT INTO votes (execution_id, voter_wallet, decision)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
            req.execution_id,
            req.voter_wallet,
            req.decision
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to cast vote");
            ApiError::InternalError("Failed to cast vote".to_string())
        })?;

        Ok(vote)
    }

    /// Get votes for an execution with pagination
    pub async fn get_votes_by_execution(
        pool: &PgPool,
        execution_id: i64,
        params: PaginationParams,
    ) -> ApiResult<PaginatedResponse<Vote>> {
        let offset = (params.page - 1) * params.limit;

        // Get total count
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM votes WHERE execution_id = $1",
            execution_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to count votes");
            ApiError::InternalError("Failed to count votes".to_string())
        })?
        .unwrap_or(0);

        // Get paginated results
        let votes = sqlx::query_as!(
            Vote,
            r#"
            SELECT * FROM votes
            WHERE execution_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            execution_id,
            params.limit as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to get votes");
            ApiError::InternalError("Failed to get votes".to_string())
        })?;

        Ok(PaginatedResponse::new(
            votes,
            params.page,
            params.limit,
            total as u64,
        ))
    }

    /// Get all votes with pagination
    pub async fn get_all_votes(
        pool: &PgPool,
        params: PaginationParams,
    ) -> ApiResult<PaginatedResponse<Vote>> {
        let offset = (params.page - 1) * params.limit;

        // Get total count
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM votes"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to count votes");
            ApiError::InternalError("Failed to count votes".to_string())
        })?
        .unwrap_or(0);

        // Get paginated results
        let votes = sqlx::query_as!(
            Vote,
            r#"
            SELECT * FROM votes
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            params.limit as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get votes");
            ApiError::InternalError("Failed to get votes".to_string())
        })?;

        Ok(PaginatedResponse::new(
            votes,
            params.page,
            params.limit,
            total as u64,
        ))
    }

    /// Get vote tally for an execution
    pub async fn get_vote_tally(
        pool: &PgPool,
        execution_id: i64,
    ) -> ApiResult<VoteTally> {
        // Get vote counts
        let vote_counts = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_votes,
                COUNT(CASE WHEN decision = 'APPROVE' THEN 1 END) as approve_votes,
                COUNT(CASE WHEN decision = 'REJECT' THEN 1 END) as reject_votes
            FROM votes
            WHERE execution_id = $1
            "#,
            execution_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to get vote counts");
            ApiError::InternalError("Failed to get vote counts".to_string())
        })?;

        // Get total committee members count
        let total_committee_members = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM committee_members WHERE is_active = true"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get committee member count");
            ApiError::InternalError("Failed to get member count".to_string())
        })?
        .unwrap_or(0);

        let total_votes = vote_counts.total_votes.unwrap_or(0);
        let approve_votes = vote_counts.approve_votes.unwrap_or(0);
        let reject_votes = vote_counts.reject_votes.unwrap_or(0);

        // Determine if voting is complete (majority of committee members have voted)
        let is_complete = total_votes >= (total_committee_members + 1) / 2; // Majority threshold
        let is_approved = approve_votes > reject_votes;

        Ok(VoteTally {
            execution_id,
            total_votes,
            approve_votes,
            reject_votes,
            total_committee_members,
            is_complete,
            is_approved,
        })
    }

    /// Check if voting is complete for an execution
    pub async fn is_voting_complete(
        pool: &PgPool,
        execution_id: i64,
    ) -> ApiResult<bool> {
        let tally = Self::get_vote_tally(pool, execution_id).await?;
        Ok(tally.is_complete)
    }

    /// Check if execution is approved by votes
    pub async fn is_execution_approved(
        pool: &PgPool,
        execution_id: i64,
    ) -> ApiResult<bool> {
        let tally = Self::get_vote_tally(pool, execution_id).await?;
        Ok(tally.is_complete && tally.is_approved)
    }

    /// Get vote by voter and execution
    pub async fn get_vote_by_voter_and_execution(
        pool: &PgPool,
        execution_id: i64,
        voter_wallet: &str,
    ) -> ApiResult<Option<Vote>> {
        let vote = sqlx::query_as!(
            Vote,
            "SELECT * FROM votes WHERE execution_id = $1 AND voter_wallet = $2",
            execution_id,
            voter_wallet
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, voter = %voter_wallet, "Failed to get vote");
            ApiError::InternalError("Failed to get vote".to_string())
        })?;

        Ok(vote)
    }

    /// Get executions that need vote completion check
    pub async fn get_executions_needing_vote_check(
        pool: &PgPool,
    ) -> ApiResult<Vec<i64>> {
        // Get executions that are in REVIEWING status and have votes but not yet finalized
        let execution_ids = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT ae.id
            FROM algorithm_executions ae
            WHERE ae.review_status = 'REVIEWING'
              AND EXISTS (SELECT 1 FROM votes v WHERE v.execution_id = ae.id)
              AND ae.vote_end_time IS NULL
            "#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get executions needing vote check");
            ApiError::InternalError("Failed to get executions".to_string())
        })?;

        Ok(execution_ids)
    }

    // Duplicate method removed - using the one above that checks vote tally

    /// Get votes for a specific execution with pagination (helper method for handlers)
    pub async fn get_votes_for_execution(
        pool: &PgPool,
        execution_id: i32,
        page: i32,
        page_size: i32,
    ) -> ApiResult<Vec<Vote>> {
        let offset = (page - 1) * page_size;

        let votes = sqlx::query_as!(
            Vote,
            r#"
            SELECT * FROM votes
            WHERE execution_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            execution_id as i64,
            page_size as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to get votes for execution");
            ApiError::InternalError("Failed to get votes".to_string())
        })?;

        Ok(votes)
    }

    /// Get votes by voter with pagination (helper method for handlers)
    pub async fn get_votes_by_voter(
        pool: &PgPool,
        voter_wallet: &str,
        page: i32,
        page_size: i32,
    ) -> ApiResult<Vec<Vote>> {
        let offset = (page - 1) * page_size;

        let votes = sqlx::query_as!(
            Vote,
            r#"
            SELECT * FROM votes
            WHERE voter_wallet = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            voter_wallet,
            page_size as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, voter = %voter_wallet, "Failed to get votes by voter");
            ApiError::InternalError("Failed to get votes".to_string())
        })?;

        Ok(votes)
    }

    /// Check if a voter has voted on a specific execution
    pub async fn has_voter_voted(
        pool: &PgPool,
        execution_id: i32,
        voter_wallet: &str,
    ) -> ApiResult<bool> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM votes WHERE execution_id = $1 AND voter_wallet = $2",
            execution_id as i64,
            voter_wallet
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, voter = %voter_wallet, "Failed to check if voter has voted");
            ApiError::InternalError("Failed to check vote status".to_string())
        })?
        .unwrap_or(0);

        Ok(count > 0)
    }
} 