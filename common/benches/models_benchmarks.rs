//! Benchmarks for shared data models in the common crate
//!
//! This benchmark suite measures the performance of serialization and deserialization
//! operations for data models used across all DeLong Protocol services.

use common::models::{
    AlgoExeData, AlgoExeStatus, AlgoExeSubmissionRequest, AlgoExeSubmissionResponse,
    AlgoReviewStatus, PaginatedResponse, PaginationParams,
};
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn create_sample_algo_exe_request() -> AlgoExeSubmissionRequest {
    AlgoExeSubmissionRequest {
        github_repo: "https://github.com/delong/longevity-algorithm".to_string(),
        commit_hash: "a1b2c3d4e5f6789012345678901234567890abcd".to_string(),
        scientist_wallet: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        dataset: "longevity_biomarkers_v2.3.1".to_string(),
    }
}

fn create_sample_algo_exe_data() -> AlgoExeData {
    AlgoExeData {
        id: 12345,
        algo_id: "algo_longevity_001".to_string(),
        used_dataset: "longevity_biomarkers_v2.3.1".to_string(),
        scientist_wallet: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        review_status: "approved".to_string(),
        vote_start_time: Some("2024-01-15T10:30:00Z".to_string()),
        vote_end_time: Some("2024-01-22T10:30:00Z".to_string()),
        status: "completed".to_string(),
        start_time: Some("2024-01-23T14:20:15Z".to_string()),
        end_time: Some("2024-01-23T16:45:32Z".to_string()),
        result: Some("{\"correlation_score\": 0.87, \"p_value\": 0.0023, \"confidence_interval\": [0.75, 0.94]}".to_string()),
        error_msg: None,
        created_at: "2024-01-15T09:15:30Z".to_string(),
        updated_at: "2024-01-23T16:45:35Z".to_string(),
        algo_name: Some("Longevity Biomarker Correlation Analysis".to_string()),
        algo_link: Some("https://github.com/delong/longevity-algorithm".to_string()),
        cid: Some("QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG".to_string()),
    }
}

fn create_sample_paginated_response() -> PaginatedResponse<AlgoExeData> {
    let items = vec![
        create_sample_algo_exe_data(),
        {
            let mut data = create_sample_algo_exe_data();
            data.id = 12346;
            data.algo_id = "algo_longevity_002".to_string();
            data
        },
        {
            let mut data = create_sample_algo_exe_data();
            data.id = 12347;
            data.algo_id = "algo_longevity_003".to_string();
            data
        },
    ];
    PaginatedResponse::new(items, 1, 20, 150)
}

fn bench_algo_exe_request_serialization(c: &mut Criterion) {
    let request = create_sample_algo_exe_request();

    c.bench_function("algo_exe_request_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&request)).unwrap())
    });
}

fn bench_algo_exe_request_deserialization(c: &mut Criterion) {
    let request = create_sample_algo_exe_request();
    let json = serde_json::to_string(&request).unwrap();

    c.bench_function("algo_exe_request_deserialize", |b| {
        b.iter(|| serde_json::from_str::<AlgoExeSubmissionRequest>(black_box(&json)).unwrap())
    });
}

fn bench_algo_exe_data_serialization(c: &mut Criterion) {
    let data = create_sample_algo_exe_data();

    c.bench_function("algo_exe_data_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&data)).unwrap())
    });
}

fn bench_algo_exe_data_deserialization(c: &mut Criterion) {
    let data = create_sample_algo_exe_data();
    let json = serde_json::to_string(&data).unwrap();

    c.bench_function("algo_exe_data_deserialize", |b| {
        b.iter(|| serde_json::from_str::<AlgoExeData>(black_box(&json)).unwrap())
    });
}

fn bench_algo_exe_response_serialization(c: &mut Criterion) {
    let response = AlgoExeSubmissionResponse { id: 12345 };

    c.bench_function("algo_exe_response_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&response)).unwrap())
    });
}

fn bench_algo_exe_response_deserialization(c: &mut Criterion) {
    let response = AlgoExeSubmissionResponse { id: 12345 };
    let json = serde_json::to_string(&response).unwrap();

    c.bench_function("algo_exe_response_deserialize", |b| {
        b.iter(|| serde_json::from_str::<AlgoExeSubmissionResponse>(black_box(&json)).unwrap())
    });
}

fn bench_pagination_params_serialization(c: &mut Criterion) {
    let params = PaginationParams::new(5, 50).unwrap();

    c.bench_function("pagination_params_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&params)).unwrap())
    });
}

fn bench_pagination_params_deserialization(c: &mut Criterion) {
    let params = PaginationParams::new(5, 50).unwrap();
    let json = serde_json::to_string(&params).unwrap();

    c.bench_function("pagination_params_deserialize", |b| {
        b.iter(|| serde_json::from_str::<PaginationParams>(black_box(&json)).unwrap())
    });
}

fn bench_paginated_response_serialization(c: &mut Criterion) {
    let response = create_sample_paginated_response();

    c.bench_function("paginated_response_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&response)).unwrap())
    });
}

fn bench_paginated_response_deserialization(c: &mut Criterion) {
    let response = create_sample_paginated_response();
    let json = serde_json::to_string(&response).unwrap();

    c.bench_function("paginated_response_deserialize", |b| {
        b.iter(|| serde_json::from_str::<PaginatedResponse<AlgoExeData>>(black_box(&json)).unwrap())
    });
}

fn bench_status_enum_serialization(c: &mut Criterion) {
    let status = AlgoExeStatus::Running;
    let review_status = AlgoReviewStatus::Approved;

    c.bench_function("algo_exe_status_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&status)).unwrap())
    });

    c.bench_function("algo_review_status_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&review_status)).unwrap())
    });
}

fn bench_status_enum_deserialization(c: &mut Criterion) {
    let status_json = serde_json::to_string(&AlgoExeStatus::Running).unwrap();
    let review_status_json = serde_json::to_string(&AlgoReviewStatus::Approved).unwrap();

    c.bench_function("algo_exe_status_deserialize", |b| {
        b.iter(|| serde_json::from_str::<AlgoExeStatus>(black_box(&status_json)).unwrap())
    });

    c.bench_function("algo_review_status_deserialize", |b| {
        b.iter(|| serde_json::from_str::<AlgoReviewStatus>(black_box(&review_status_json)).unwrap())
    });
}

fn bench_pagination_operations(c: &mut Criterion) {
    c.bench_function("pagination_params_offset_calculation", |b| {
        b.iter(|| {
            let params = PaginationParams::new(black_box(10), black_box(25)).unwrap();
            black_box(params.offset())
        })
    });

    c.bench_function("pagination_params_validation", |b| {
        b.iter(|| {
            let params = PaginationParams::new(black_box(5), black_box(20)).unwrap();
            black_box(params.validate())
        })
    });

    c.bench_function("paginated_response_creation", |b| {
        b.iter(|| {
            let items = vec![
                create_sample_algo_exe_data(),
                create_sample_algo_exe_data(),
                create_sample_algo_exe_data(),
            ];
            black_box(PaginatedResponse::new(items, 1, 20, 100))
        })
    });
}

criterion_group!(
    models_benchmarks,
    bench_algo_exe_request_serialization,
    bench_algo_exe_request_deserialization,
    bench_algo_exe_data_serialization,
    bench_algo_exe_data_deserialization,
    bench_algo_exe_response_serialization,
    bench_algo_exe_response_deserialization,
    bench_pagination_params_serialization,
    bench_pagination_params_deserialization,
    bench_paginated_response_serialization,
    bench_paginated_response_deserialization,
    bench_status_enum_serialization,
    bench_status_enum_deserialization,
    bench_pagination_operations
);

criterion_main!(models_benchmarks);
