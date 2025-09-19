//! Event handlers for blockchain events
//!
//! This module contains the actual event handling logic,
//! extracted from mod.rs to keep it clean and focused.

use crate::{
    error::{Result as AppResult, TimestampExt},
    models::{
        blockchain_transaction::BlockchainTransaction, dataset::Dataset, FindById, ReviewStatus,
    },
    AppError,
};
use alloy::primitives::{Address, U256};
use alloy::rpc::types::Log;
use tracing::{error, info, warn};

use super::ChainSyncWorker;

impl ChainSyncWorker {
    /// Handle DataRegistered event
    pub async fn handle_data_registered(
        &self,
        log: &Log,
        contributor: Address,
        cid: String,
        dataset_id: U256,
        dataset: String,
    ) -> AppResult<()> {
        info!(
            "DataRegistered: contributor={:?}, cid={}, dataset={}",
            contributor, cid, dataset
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        let tx_hash_str = format!("{:?}", tx_hash);

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        // Convert dataset_id from U256 to i64
        let db_dataset_id = dataset_id.to::<i64>();

        // Find dataset by ID
        let dataset_record = Dataset::find_by_id(self.db.pool(), db_dataset_id).await?;

        let entity_id = match dataset_record {
            Some(ds) => ds.id,
            None => {
                warn!(
                    "Dataset not found by ID {} for transaction {}",
                    db_dataset_id, tx_hash_str
                );
                0 // Use 0 as placeholder
            }
        };

        // Update transaction status
        let status = crate::models::TransactionStatus::Confirmed;

        // Upsert transaction record
        let was_updated = BlockchainTransaction::upsert_from_event(
            self.db.pool(),
            &tx_hash_str,
            entity_id,
            crate::models::blockchain_transaction::EntityType::Dataset,
            status,
            block_number as i64,
            block_timestamp,
        )
        .await?;

        if was_updated {
            info!("Updated blockchain transaction: tx={}", tx_hash_str);
        } else {
            info!("Created blockchain transaction: tx={}", tx_hash_str);
        }

        // Fetch updated transaction and notify
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send notification: {}", e);
        }

        Ok(())
    }

    /// Handle AlgorithmSubmitted event
    pub async fn handle_algorithm_submitted(
        &self,
        log: &Log,
        submitter: Address,
        cid: String,
        algo_id: U256,
        algorithm: String,
    ) -> AppResult<()> {
        info!(
            "AlgorithmSubmitted: submitter={:?}, cid={}, algorithm={}",
            submitter, cid, algorithm
        );

        // Get transaction info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        let tx_hash_str = format!("{:?}", tx_hash);
        let db_algo_id = algo_id.to::<i64>();

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        // Update transaction status
        let status = crate::models::TransactionStatus::Confirmed;

        let was_updated = BlockchainTransaction::upsert_from_event(
            self.db.pool(),
            &tx_hash_str,
            db_algo_id,
            crate::models::blockchain_transaction::EntityType::Execution,
            status,
            block_number as i64,
            block_timestamp,
        )
        .await?;

        if was_updated {
            info!(
                "Updated algorithm submission transaction: tx={}",
                tx_hash_str
            );
        } else {
            info!(
                "Created algorithm submission transaction: tx={}",
                tx_hash_str
            );
        }

        // Fetch and notify
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send notification: {}", e);
        }

        Ok(())
    }

    /// Handle AlgorithmResolved event
    pub async fn handle_algorithm_resolved(
        &self,
        log: &Log,
        execution_id: U256,
        cid: String,
        approved: bool,
    ) -> AppResult<()> {
        info!(
            "AlgorithmResolved: execution_id={}, cid={}, approved={}",
            execution_id, cid, approved
        );

        // Get transaction hash
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;

        info!("Received event tx={:?}", tx_hash);

        // Get block info
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        let exe_id = execution_id.to::<i64>();
        let tx_hash_str = format!("{:?}", tx_hash);

        // Update algorithm execution status
        let status = if approved {
            ReviewStatus::Approved
        } else {
            ReviewStatus::Rejected
        };

        // Update algorithm_execution table
        sqlx::query!(
            r#"
            UPDATE algorithm_execution
            SET review_status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            exe_id,
            if approved { "approved" } else { "rejected" }
        )
        .execute(self.db.pool())
        .await?;

        // Also update old execution table for backward compatibility
        sqlx::query!(
            r#"
            UPDATE execution
            SET review_status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            exe_id,
            status as ReviewStatus
        )
        .execute(self.db.pool())
        .await
        .ok(); // Ignore errors for old table

        // Create/update blockchain transaction record
        let tx_status = crate::models::TransactionStatus::Confirmed;

        let was_updated = BlockchainTransaction::upsert_from_event(
            self.db.pool(),
            &tx_hash_str,
            exe_id,
            crate::models::blockchain_transaction::EntityType::Execution,
            tx_status,
            block_number as i64,
            block_timestamp,
        )
        .await?;

        if was_updated {
            info!(
                "Updated blockchain transaction for algorithm resolution: tx={}",
                tx_hash_str
            );
        } else {
            info!(
                "Created blockchain transaction for algorithm resolution: tx={}",
                tx_hash_str
            );
        }

        // Fetch the updated transaction record
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str.clone(), &transaction)
            .await
        {
            warn!("Failed to send algorithm resolution notification: {}", e);
        }

        // If approved, schedule algorithm execution
        if approved {
            info!(
                "Execution task {} approved, notifying runtime service",
                exe_id
            );
            // Schedule the algorithm execution using new execute_task method
            if let Err(e) = self.executor.execute_task(exe_id).await {
                error!(
                    "Failed to schedule algorithm execution for {}: {}",
                    exe_id, e
                );
            }
        } else {
            info!("Execution task {} rejected", exe_id);
        }

        Ok(())
    }

    /// Handle ExecutionSubmitted event (AI audit already passed)
    pub async fn handle_execution_submitted(
        &self,
        log: &Log,
        execution_id: U256,
        cid: String,
        start_time: U256,
        end_time: U256,
    ) -> AppResult<()> {
        info!(
            "ExecutionSubmitted: execution_id={}, cid={} - will execute immediately (AI audit already passed)",
            execution_id, cid
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        // Update transaction status
        let status = crate::models::TransactionStatus::Confirmed;
        let tx_hash_str = format!("{:?}", tx_hash);
        let exe_id = execution_id.to::<i64>();

        // Upsert transaction record
        let was_updated = BlockchainTransaction::upsert_from_event(
            self.db.pool(),
            &tx_hash_str,
            exe_id,
            crate::models::blockchain_transaction::EntityType::Execution,
            status,
            block_number as i64,
            block_timestamp,
        )
        .await?;

        if was_updated {
            info!(
                "Updated blockchain transaction for execution submission: tx={}",
                tx_hash_str
            );
        } else {
            info!(
                "Created blockchain transaction for execution submission: tx={}",
                tx_hash_str
            );
        }

        // Convert timestamps to DateTime
        let vote_start_naive = (start_time.to::<u64>() as i64).to_naive_datetime()?;
        let vote_start = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            vote_start_naive,
            chrono::Utc,
        );
        let vote_end_naive = (end_time.to::<u64>() as i64).to_naive_datetime()?;
        let vote_end =
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(vote_end_naive, chrono::Utc);

        // Update execution record in new algorithm_execution table
        if status == crate::models::TransactionStatus::Confirmed {
            // Update the new table
            sqlx::query!(
                r#"
                UPDATE algorithm_execution
                SET review_status = 'approved',
                    execution_status = 'queued',
                    vote_start_time = $2,
                    vote_end_time = $3,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                exe_id,
                vote_start,
                vote_end
            )
            .execute(self.db.pool())
            .await?;

            // Also update old table for backward compatibility (will be removed later)
            sqlx::query!(
                r#"
                UPDATE execution
                SET vote_start_time = $2, vote_end_time = $3, updated_at = NOW()
                WHERE id = $1
                "#,
                exe_id,
                vote_start,
                vote_end
            )
            .execute(self.db.pool())
            .await
            .ok(); // Ignore errors for old table

            // Directly schedule algorithm execution (no need to wait for voting)
            // AI audit already passed during submission
            info!(
                "Directly scheduling algorithm execution {} (cid: {}) - AI audit already passed",
                exe_id, cid
            );

            // Schedule execution using new execute_task method
            if let Err(e) = self.executor.execute_task(exe_id).await {
                error!(
                    "Failed to schedule algorithm execution for {} (cid: {}): {}",
                    exe_id, cid, e
                );
            }
        }

        // Fetch the updated transaction and send result
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send execution submission notification: {}", e);
        }

        Ok(())
    }

    /// Handle CommitteeMemberUpdated event
    pub async fn handle_committee_member_updated(
        &self,
        log: &Log,
        member: Address,
        approved: bool,
    ) -> AppResult<()> {
        info!(
            "CommitteeMemberUpdated: member={:?}, approved={}",
            member, approved
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        // Update transaction status
        let status = crate::models::TransactionStatus::Confirmed;
        let tx_hash_str = format!("{:?}", tx_hash);

        // Update committee member
        let member_wallet = format!("{:?}", member);
        let committee_member = sqlx::query!(
            r#"
            INSERT INTO committee (wallet, is_approved)
            VALUES ($1, $2)
            ON CONFLICT (wallet)
            DO UPDATE SET is_approved = $2, updated_at = NOW()
            RETURNING id
            "#,
            member_wallet,
            approved
        )
        .fetch_one(self.db.pool())
        .await?;

        // Create/update blockchain transaction record
        let was_updated = BlockchainTransaction::upsert_from_event(
            self.db.pool(),
            &tx_hash_str,
            committee_member.id as i64,
            crate::models::blockchain_transaction::EntityType::Committee,
            status,
            block_number as i64,
            block_timestamp,
        )
        .await?;

        if was_updated {
            info!(
                "Updated blockchain transaction for committee update: tx={}",
                tx_hash_str
            );
        } else {
            info!(
                "Created blockchain transaction for committee update: tx={}",
                tx_hash_str
            );
        }

        // Fetch the updated transaction
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send committee update notification: {}", e);
        }

        Ok(())
    }

    /// Handle VoteCasted event
    pub async fn handle_vote_casted(
        &self,
        log: &Log,
        member: Address,
        cid: String,
        approved: bool,
        vote_time: U256,
    ) -> AppResult<()> {
        info!(
            "VoteCasted: member={:?}, cid={}, approved={}, vote_time={}",
            member, cid, approved, vote_time
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        let voted_at_naive = (vote_time.to::<u64>() as i64).to_naive_datetime()?;
        let voted_at =
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(voted_at_naive, chrono::Utc);

        // Format tx_hash once for consistency
        let tx_hash_str = format!("{:?}", tx_hash);

        // Begin transaction to ensure atomicity
        let mut tx = self.db.pool().begin().await?;

        // Store the vote
        let vote = sqlx::query!(
            r#"
            INSERT INTO vote (algo_cid, wallet, approved, vote_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (algo_cid, wallet)
            DO UPDATE SET approved = $3, vote_at = $4
            RETURNING id
            "#,
            &cid,
            format!("{:?}", member),
            approved,
            voted_at
        )
        .fetch_one(&mut *tx)
        .await?;

        // Create blockchain transaction record with VOTE entity type
        let status = crate::models::TransactionStatus::Confirmed;
        let entity_type = crate::models::EntityType::Vote;

        sqlx::query!(
            r#"
            INSERT INTO transaction (tx_hash, entity_id, entity_type, status, block_number, block_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (tx_hash)
            DO UPDATE SET
                status = EXCLUDED.status,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                updated_at = NOW()
            "#,
            tx_hash_str.clone(),
            vote.id,
            entity_type as crate::models::EntityType,
            status as crate::models::TransactionStatus,
            block_number as i64,
            block_timestamp
        )
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        info!(
            "Vote recorded: id={}, algo_cid={}, voter={:?}, approved={}",
            vote.id, cid, member, approved
        );

        // Fetch the created/updated transaction record
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send vote notification: {}", e);
        }

        Ok(())
    }
}
