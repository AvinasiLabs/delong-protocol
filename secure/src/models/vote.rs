use avinapi::prelude::AppResult as Result;
use avinapi::{query::PaginationQuery, transport::response::PaginatedData};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{Create, FindById, Timestamped};

/// Vote entity - represents a committee member's vote on an algorithm
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Vote {
    pub id: i64,
    pub algo_cid: String,
    pub voter: String,
    pub approve: bool,
    pub voted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Vote {
    /// Find all votes for a specific algorithm CID
    pub async fn find_by_algo_cid(pool: &PgPool, algo_cid: &str) -> Result<Vec<Self>> {
        let votes = sqlx::query_as!(
            Vote,
            r#"
            SELECT * FROM votes
            WHERE algo_cid = $1
            ORDER BY voted_at DESC
            "#,
            algo_cid
        )
        .fetch_all(pool)
        .await?;

        Ok(votes)
    }

    /// Find votes by algorithm CID with pagination
    pub async fn find_by_algo_cid_paginated(
        pool: &PgPool,
        algo_cid: &str,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<Self>, u64)> {
        // Get total count
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM votes WHERE algo_cid = $1",
            algo_cid
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let votes = sqlx::query_as!(
            Vote,
            r#"
            SELECT * FROM votes
            WHERE algo_cid = $1
            ORDER BY voted_at DESC
            LIMIT $2 OFFSET $3
            "#,
            algo_cid,
            per_page as i64,
            ((page - 1) * per_page) as i64
        )
        .fetch_all(pool)
        .await?;

        Ok((votes, total as u64))
    }

    /// Count votes for a specific algorithm CID
    pub async fn count_by_algo_cid(pool: &PgPool, algo_cid: &str) -> Result<(i64, i64)> {
        let result = sqlx::query!(
            r#"
            SELECT
                COUNT(CASE WHEN approve = true THEN 1 END) as approve_count,
                COUNT(CASE WHEN approve = false THEN 1 END) as reject_count
            FROM votes
            WHERE algo_cid = $1
            "#,
            algo_cid
        )
        .fetch_one(pool)
        .await?;

        Ok((
            result.approve_count.unwrap_or(0),
            result.reject_count.unwrap_or(0),
        ))
    }

    /// Find votes by voter with pagination
    pub async fn find_by_voter_paginated(
        pool: &PgPool,
        voter: &str,
        pagination: PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        // Get total count
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM votes WHERE voter = $1",
            voter
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let votes = sqlx::query_as!(
            Vote,
            r#"
            SELECT * FROM votes
            WHERE voter = $1
            ORDER BY voted_at DESC
            LIMIT $2 OFFSET $3
            "#,
            voter,
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: votes,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Find a vote by algo_cid and voter (unique combination)
    pub async fn find_by_algo_cid_and_voter(
        pool: &PgPool,
        algo_cid: &str,
        voter: &str,
    ) -> Result<Option<Self>> {
        let vote = sqlx::query_as!(
            Vote,
            "SELECT * FROM votes WHERE algo_cid = $1 AND voter = $2",
            algo_cid,
            voter
        )
        .fetch_optional(pool)
        .await?;

        Ok(vote)
    }

    /// Check if a voter has already voted on an algorithm
    pub async fn has_voted(pool: &PgPool, algo_cid: &str, voter: &str) -> Result<bool> {
        let exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM votes WHERE algo_cid = $1 AND voter = $2)",
            algo_cid,
            voter
        )
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }

    /// Get all votes within a time range with pagination
    pub async fn find_by_time_range_paginated(
        pool: &PgPool,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        pagination: PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        // Get total count
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM votes WHERE voted_at BETWEEN $1 AND $2",
            start_time,
            end_time
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let votes = sqlx::query_as!(
            Vote,
            r#"
            SELECT * FROM votes
            WHERE voted_at BETWEEN $1 AND $2
            ORDER BY voted_at DESC
            LIMIT $3 OFFSET $4
            "#,
            start_time,
            end_time,
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: votes,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Create a new vote within a database transaction
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cid: &str,
        member: &str,
        approved: bool,
        vote_time: DateTime<Utc>,
    ) -> Result<Self> {
        let vote = sqlx::query_as!(
            Vote,
            r#"
            INSERT INTO votes (algo_cid, voter, approve, voted_at)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            cid,
            member,
            approved,
            vote_time
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(vote)
    }
}

impl Timestamped for Vote {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

/// Request to create a new vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVoteRequest {
    pub algo_cid: String,
    pub voter: String,
    pub approve: bool,
    pub voted_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Create for Vote {
    type Request = CreateVoteRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let vote = sqlx::query_as!(
            Vote,
            r#"
            INSERT INTO votes (algo_cid, voter, approve, voted_at)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            &request.algo_cid,
            &request.voter,
            request.approve,
            request.voted_at
        )
        .fetch_one(pool)
        .await?;

        Ok(vote)
    }
}

#[async_trait::async_trait]
impl FindById for Vote {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let vote = sqlx::query_as!(Vote, "SELECT * FROM votes WHERE id = $1", id)
            .fetch_optional(pool)
            .await?;

        Ok(vote)
    }
}

#[cfg(test)]
mod tests {

    // Add tests here if needed
}
