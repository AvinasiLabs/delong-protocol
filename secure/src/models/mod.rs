pub mod algo;
pub mod algo_exe;

pub mod blockchain_transaction;
pub mod committee;
pub mod contract;
pub mod data_usage;
pub mod dataset;
pub mod pg_types;
pub mod vote;

// Re-export all model types
pub use algo::{Algo, CreateAlgo};
pub use algo_exe::{AlgoExe, AlgoExeWithAlgo, CreateAlgoExeRequest};

pub use blockchain_transaction::{BlockchainTransaction, EntityType};
pub use committee::{CommitteeMember, CreateCommitteeMemberRequest};
pub use contract::{ContractMeta, CreateContractMetaRequest};
pub use data_usage::{CreateDataUsageRequest, DataUsage};
pub use dataset::{CreateDatasetRequest, Dataset};
pub use pg_types::{AlgoExeStatus, AlgoReviewStatus, TransactionStatus};
pub use vote::{CreateVoteRequest, Vote};

// Common model traits
use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Common trait for models with timestamps
pub trait Timestamped {
    fn created_at(&self) -> &DateTime<Utc>;
    fn updated_at(&self) -> &DateTime<Utc>;
}

/// Common trait for models that can be created
#[async_trait::async_trait]
pub trait Create: Sized {
    type Request;

    async fn create(pool: &PgPool, request: Self::Request) -> crate::Result<Self>;
}

/// Common trait for models that can be found by ID
#[async_trait::async_trait]
pub trait FindById: Sized {
    async fn find_by_id(pool: &PgPool, id: i64) -> crate::Result<Option<Self>>;

    async fn find_by_id_required(pool: &PgPool, id: i64) -> crate::Result<Self> {
        Self::find_by_id(pool, id).await?.ok_or_else(|| {
            crate::AppError::NotFound(format!(
                "{} with id {} not found",
                std::any::type_name::<Self>(),
                id
            ))
        })
    }
}
