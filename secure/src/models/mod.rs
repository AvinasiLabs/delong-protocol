pub mod algorithm;
pub mod algorithm_execution;
pub mod blockchain_transaction;
pub mod committee;
pub mod contract;
pub mod dataset;
pub mod dataset_schema;
pub mod dataset_tag;
pub mod vote;

// Re-export all model types
pub use algorithm::{Algo, CreateAlgo};
pub use algorithm_execution::{
    AlgorithmExecution, ContainerResult, CreateAlgorithmExecutionRequest, ExecutionContext,
    ExecutionStats, ExecutionStatus, ReviewStatus,
};
pub use blockchain_transaction::TransactionStatus;
pub use blockchain_transaction::{BlockchainTransaction, EntityType};
pub use committee::{Committee, CreateCommitteeMemberRequest};
pub use contract::{Contract, CreateContractMetaRequest};
pub use dataset::{CreateDatasetRequest, Dataset};
pub use dataset_schema::{
    CreateSchemaFieldRequest, DatasetSchema, SchemaFieldDefinition, SetDatasetSchemaRequest,
    UpdateSchemaFieldRequest,
};
pub use dataset_tag::{AddTagsRequest, DatasetTag, RemoveTagsRequest, TagWithCount};
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
