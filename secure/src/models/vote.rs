use common::{models::{PaginatedResponse, PaginationParams}, ApiError, Result as ApiResult, CommonError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

/// Vote database model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Vote {
    pub id: i64,
    pub execution_id: i64,
    pub voter_wallet: String,
    pub decision: String, // "APPROVE" or "REJECT"
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

/// Query parameters for listing votes
#[derive(Debug, Clone, Deserialize)]
pub struct VoteQuery {
    pub execution_id: Option<i64>,
    pub voter_wallet: Option<String>,
}

impl Vote {
    /// Create a new vote within a transaction.
    pub async fn create_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        req: &CastVoteRequest,
    ) -> Result<Self, sqlx::Error> {
        let decision_upper = req.decision.to_uppercase();
        if decision_upper != "APPROVE" && decision_upper != "REJECT" {
            // This is a validation error, but we return a generic SQLx error
            // to be handled and converted by the handler.
            return Err(sqlx::Error::Protocol(
                "Decision must be APPROVE or REJECT".into(),
            ));
        }

        sqlx::query_as(
            r#"
            INSERT INTO votes (execution_id, voter_wallet, decision)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(req.execution_id)
        .bind(&req.voter_wallet)
        .bind(decision_upper)
        .fetch_one(&mut **tx)
        .await
    }

    /// Check if a voter has already voted for a specific execution within a transaction.
    pub async fn has_voted(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        execution_id: i64,
        voter_wallet: &str,
    ) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM votes WHERE execution_id = $1 AND voter_wallet = $2",
        )
        .bind(execution_id)
        .bind(voter_wallet)
        .fetch_one(&mut **tx)
        .await?;
        Ok(count > 0)
    }

    /// Get a single vote by its ID.
    pub async fn get_by_id(pool: &PgPool, id: i64) -> common::Result<Option<Self>> {
        Ok(
            sqlx::query_as("SELECT * FROM votes WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?,
        )
    }

    /// List votes with pagination and filtering.
    pub async fn list(
        pool: &PgPool,
        page: i64,
        limit: i64,
        query: VoteQuery,
    ) -> common::Result<(Vec<Self>, i64)> {
        let offset = (page - 1) * limit;

        // Base query - Aligned with Go reference to only select confirmed votes
        let mut query_builder = sqlx::QueryBuilder::new(
            "SELECT v.* FROM votes v JOIN blockchain_transactions bt ON v.id = bt.entity_id WHERE bt.entity_type = 'VOTE' AND bt.status = 'CONFIRMED' ",
        );
        let mut count_builder = sqlx::QueryBuilder::new(
            "SELECT COUNT(v.id) FROM votes v JOIN blockchain_transactions bt ON v.id = bt.entity_id WHERE bt.entity_type = 'VOTE' AND bt.status = 'CONFIRMED' ",
        );

        if let Some(exec_id) = query.execution_id {
            query_builder.push(" AND v.execution_id = ");
            query_builder.push_bind(exec_id);
            count_builder.push(" AND v.execution_id = ");
            count_builder.push_bind(exec_id);
        }

        if let Some(voter) = query.voter_wallet {
            query_builder.push(" AND v.voter_wallet = ");
            query_builder.push_bind(voter.clone());
            count_builder.push(" AND v.voter_wallet = ");
            count_builder.push_bind(voter);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(pool).await?;

        query_builder.push(" ORDER BY v.created_at DESC LIMIT ");
        query_builder.push_bind(limit);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset);

        let votes = query_builder.build_query_as().fetch_all(pool).await?;

        Ok((votes, total.0))
    }
} 