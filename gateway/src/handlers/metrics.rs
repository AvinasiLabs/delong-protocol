// //! Metrics handler for basic system monitoring
// //!
// //! This module provides simplified metrics collection and reporting
// //! for monitoring the gateway's performance and health status.

// use axum::{
//     http::StatusCode,
//     response::{IntoResponse, Response},
// };
// use serde::Serialize;
// use std::{
//     collections::HashMap,
//     sync::{Arc, Mutex},
//     time::{Duration, Instant},
// };
// use tracing::{info, instrument};

// use crate::utils::generate_request_id;

// /// Simple metrics collector
// #[derive(Debug, Clone)]
// pub struct MetricsCollector {
//     pub start_time: Instant,
//     pub request_count: Arc<Mutex<u64>>,
//     pub error_count: Arc<Mutex<u64>>,
//     pub response_times: Arc<Mutex<Vec<Duration>>>,
//     pub endpoint_stats: Arc<Mutex<HashMap<String, EndpointMetrics>>>,
// }

// /// Per-endpoint metrics
// #[derive(Debug, Clone, Default)]
// pub struct EndpointMetrics {
//     pub total_requests: u64,
//     pub error_count: u64,
//     pub total_response_time_ms: u64,
//     pub min_response_time_ms: u64,
//     pub max_response_time_ms: u64,
// }

// /// Basic system metrics
// #[derive(Debug, Serialize)]
// pub struct SystemMetrics {
//     pub uptime_seconds: u64,
//     pub total_requests: u64,
//     pub error_count: u64,
//     pub error_rate: f64,
//     pub average_response_time_ms: f64,
//     pub requests_per_minute: f64,
//     pub timestamp: String,
//     pub version: String,
//     pub build_info: BuildInfo,
// }

// /// Build information
// #[derive(Debug, Serialize)]
// pub struct BuildInfo {
//     pub version: String,
//     pub commit_hash: Option<String>,
//     pub build_date: Option<String>,
//     pub rust_version: String,
// }

// /// Prometheus-style metrics response
// pub struct PrometheusMetrics {
//     pub content: String,
// }

// impl IntoResponse for PrometheusMetrics {
//     fn into_response(self) -> Response {
//         (
//             StatusCode::OK,
//             [("content-type", "text/plain; charset=utf-8")],
//             self.content,
//         )
//             .into_response()
//     }
// }

// impl Default for MetricsCollector {
//     fn default() -> Self {
//         Self {
//             start_time: Instant::now(),
//             request_count: Arc::new(Mutex::new(0)),
//             error_count: Arc::new(Mutex::new(0)),
//             response_times: Arc::new(Mutex::new(Vec::new())),
//             endpoint_stats: Arc::new(Mutex::new(HashMap::new())),
//         }
//     }
// }

// impl MetricsCollector {
//     /// Record a successful request
//     pub fn record_request(&self, endpoint: &str, response_time: Duration) {
//         // Update total request count
//         if let Ok(mut count) = self.request_count.lock() {
//             *count += 1;
//         }

//         // Record response time
//         if let Ok(mut times) = self.response_times.lock() {
//             times.push(response_time);
//             // Keep only last 1000 response times to avoid memory growth
//             if times.len() > 1000 {
//                 let len = times.len();
//                 times.drain(0..len - 1000);
//             }
//         }

//         // Update endpoint-specific metrics
//         if let Ok(mut stats) = self.endpoint_stats.lock() {
//             let endpoint_metric = stats.entry(endpoint.to_string()).or_default();
//             endpoint_metric.total_requests += 1;

//             let response_time_ms = response_time.as_millis() as u64;
//             endpoint_metric.total_response_time_ms += response_time_ms;

//             if endpoint_metric.min_response_time_ms == 0
//                 || response_time_ms < endpoint_metric.min_response_time_ms
//             {
//                 endpoint_metric.min_response_time_ms = response_time_ms;
//             }

//             if response_time_ms > endpoint_metric.max_response_time_ms {
//                 endpoint_metric.max_response_time_ms = response_time_ms;
//             }
//         }
//     }

//     /// Record an error
//     pub fn record_error(&self, endpoint: &str) {
//         // Update total error count
//         if let Ok(mut count) = self.error_count.lock() {
//             *count += 1;
//         }

//         // Update endpoint-specific error count
//         if let Ok(mut stats) = self.endpoint_stats.lock() {
//             let endpoint_metric = stats.entry(endpoint.to_string()).or_default();
//             endpoint_metric.error_count += 1;
//         }
//     }

//     /// Get current system metrics
//     pub fn get_system_metrics(&self) -> SystemMetrics {
//         let uptime = self.start_time.elapsed().as_secs();
//         let total_requests = self.request_count.lock().map(|guard| *guard).unwrap_or(0);
//         let error_count = self.error_count.lock().map(|guard| *guard).unwrap_or(0);

//         let error_rate = if total_requests > 0 {
//             (error_count as f64) / (total_requests as f64) * 100.0
//         } else {
//             0.0
//         };

//         let average_response_time = if let Ok(times) = self.response_times.lock() {
//             if times.is_empty() {
//                 0.0
//             } else {
//                 times.iter().map(|t| t.as_millis() as f64).sum::<f64>() / times.len() as f64
//             }
//         } else {
//             0.0
//         };

//         let requests_per_minute = if uptime > 0 {
//             (total_requests as f64) / (uptime as f64 / 60.0)
//         } else {
//             0.0
//         };

//         SystemMetrics {
//             uptime_seconds: uptime,
//             total_requests,
//             error_count,
//             error_rate,
//             average_response_time_ms: average_response_time,
//             requests_per_minute,
//             timestamp: chrono::Utc::now().to_rfc3339(),
//             version: env!("CARGO_PKG_VERSION").to_string(),
//             build_info: BuildInfo {
//                 version: env!("CARGO_PKG_VERSION").to_string(),
//                 commit_hash: option_env!("GIT_HASH").map(|s| s.to_string()),
//                 build_date: option_env!("BUILD_DATE").map(|s| s.to_string()),
//                 rust_version: "unknown".to_string(),
//             },
//         }
//     }

//     /// Generate Prometheus-format metrics
//     pub fn get_prometheus_metrics(&self) -> String {
//         let mut output = String::new();

//         // Add help text
//         output.push_str("# HELP delong_gateway_info Gateway information\n");
//         output.push_str("# TYPE delong_gateway_info gauge\n");
//         output.push_str(&format!(
//             "delong_gateway_info{{version=\"{}\"}} 1\n",
//             env!("CARGO_PKG_VERSION")
//         ));

//         // Uptime
//         output.push_str("# HELP delong_gateway_uptime_seconds Gateway uptime in seconds\n");
//         output.push_str("# TYPE delong_gateway_uptime_seconds counter\n");
//         output.push_str(&format!(
//             "delong_gateway_uptime_seconds {}\n",
//             self.start_time.elapsed().as_secs()
//         ));

//         // Request metrics
//         let total_requests = self.request_count.lock().map(|guard| *guard).unwrap_or(0);
//         output.push_str("# HELP delong_gateway_requests_total Total number of requests\n");
//         output.push_str("# TYPE delong_gateway_requests_total counter\n");
//         output.push_str(&format!(
//             "delong_gateway_requests_total {}\n",
//             total_requests
//         ));

//         // Error metrics
//         let error_count = self.error_count.lock().map(|guard| *guard).unwrap_or(0);
//         output.push_str("# HELP delong_gateway_errors_total Total number of errors\n");
//         output.push_str("# TYPE delong_gateway_errors_total counter\n");
//         output.push_str(&format!("delong_gateway_errors_total {}\n", error_count));

//         // Response time histogram (simplified)
//         if let Ok(times) = self.response_times.lock() {
//             if !times.is_empty() {
//                 let avg_time =
//                     times.iter().map(|t| t.as_millis() as f64).sum::<f64>() / times.len() as f64;
//                 output.push_str("# HELP delong_gateway_response_time_ms Average response time in milliseconds\n");
//                 output.push_str("# TYPE delong_gateway_response_time_ms gauge\n");
//                 output.push_str(&format!(
//                     "delong_gateway_response_time_ms {:.2}\n",
//                     avg_time
//                 ));
//             }
//         }

//         // Per-endpoint metrics
//         if let Ok(stats) = self.endpoint_stats.lock() {
//             for (endpoint, metrics) in stats.iter() {
//                 let sanitized_endpoint = endpoint.replace("/", "_").replace("-", "_");

//                 output.push_str(&format!(
//                     "delong_gateway_endpoint_requests_total{{endpoint=\"{}\"}} {}\n",
//                     endpoint, metrics.total_requests
//                 ));

//                 output.push_str(&format!(
//                     "delong_gateway_endpoint_errors_total{{endpoint=\"{}\"}} {}\n",
//                     endpoint, metrics.error_count
//                 ));

//                 if metrics.total_requests > 0 {
//                     let avg_response_time =
//                         metrics.total_response_time_ms as f64 / metrics.total_requests as f64;
//                     output.push_str(&format!(
//                         "delong_gateway_endpoint_response_time_ms{{endpoint=\"{}\"}} {:.2}\n",
//                         endpoint, avg_response_time
//                     ));
//                 }
//             }
//         }

//         output
//     }
// }

// // Global metrics collector instance
// lazy_static::lazy_static! {
//     static ref METRICS_COLLECTOR: MetricsCollector = MetricsCollector::default();
// }

// /// Get the global metrics collector
// pub fn get_metrics_collector() -> &'static MetricsCollector {
//     &METRICS_COLLECTOR
// }

// /// Main metrics handler - returns JSON format
// #[instrument(fields(request_id))]
// pub async fn metrics_handler() -> Result<axum::response::Json<SystemMetrics>, StatusCode> {
//     let request_id = generate_request_id();
//     tracing::Span::current().record("request_id", &request_id);

//     info!(request_id = request_id, "Metrics request received");

//     let metrics = get_metrics_collector().get_system_metrics();

//     info!(
//         request_id = request_id,
//         uptime = metrics.uptime_seconds,
//         total_requests = metrics.total_requests,
//         "Metrics retrieved successfully"
//     );

//     Ok(axum::response::Json(metrics))
// }

// /// Prometheus metrics endpoint
// #[instrument(fields(request_id))]
// pub async fn prometheus_metrics_handler() -> Result<PrometheusMetrics, StatusCode> {
//     let request_id = generate_request_id();
//     tracing::Span::current().record("request_id", &request_id);

//     info!(
//         request_id = request_id,
//         "Prometheus metrics request received"
//     );

//     let content = get_metrics_collector().get_prometheus_metrics();

//     Ok(PrometheusMetrics { content })
// }

// /// Utility function to record request metrics (called by middleware)
// pub fn record_request_metric(endpoint: &str, response_time: Duration, is_error: bool) {
//     let collector = get_metrics_collector();

//     if is_error {
//         collector.record_error(endpoint);
//     } else {
//         collector.record_request(endpoint, response_time);
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::time::Duration;

//     #[test]
//     fn test_metrics_collector_creation() {
//         let collector = MetricsCollector::default();
//         let metrics = collector.get_system_metrics();

//         assert_eq!(metrics.total_requests, 0);
//         assert_eq!(metrics.error_count, 0);
//         assert_eq!(metrics.error_rate, 0.0);
//     }

//     #[test]
//     fn test_record_request() {
//         let collector = MetricsCollector::default();

//         collector.record_request("/test", Duration::from_millis(100));
//         collector.record_request("/test", Duration::from_millis(200));

//         let metrics = collector.get_system_metrics();
//         assert_eq!(metrics.total_requests, 2);
//         assert_eq!(metrics.error_count, 0);
//         assert!(metrics.average_response_time_ms > 0.0);
//     }

//     #[test]
//     fn test_record_error() {
//         let collector = MetricsCollector::default();

//         collector.record_request("/test", Duration::from_millis(100));
//         collector.record_error("/test");

//         let metrics = collector.get_system_metrics();
//         assert_eq!(metrics.total_requests, 1);
//         assert_eq!(metrics.error_count, 1);
//         assert_eq!(metrics.error_rate, 100.0);
//     }

//     #[test]
//     fn test_prometheus_metrics_format() {
//         let collector = MetricsCollector::default();
//         collector.record_request("/api/test", Duration::from_millis(150));

//         let prometheus_output = collector.get_prometheus_metrics();

//         assert!(prometheus_output.contains("delong_gateway_info"));
//         assert!(prometheus_output.contains("delong_gateway_uptime_seconds"));
//         assert!(prometheus_output.contains("delong_gateway_requests_total"));
//         assert!(prometheus_output.contains("delong_gateway_errors_total"));
//     }

//     #[test]
//     fn test_endpoint_specific_metrics() {
//         let collector = MetricsCollector::default();

//         collector.record_request("/api/data", Duration::from_millis(100));
//         collector.record_request("/api/data", Duration::from_millis(200));
//         collector.record_error("/api/data");

//         if let Ok(stats) = collector.endpoint_stats.lock() {
//             let data_stats = stats.get("/api/data").unwrap();
//             assert_eq!(data_stats.total_requests, 2);
//             assert_eq!(data_stats.error_count, 1);
//             assert_eq!(data_stats.total_response_time_ms, 300);
//         }
//     }

//     #[test]
//     fn test_requests_per_minute_calculation() {
//         let collector = MetricsCollector::default();

//         // Simulate some requests
//         for _ in 0..10 {
//             collector.record_request("/test", Duration::from_millis(100));
//         }

//         let metrics = collector.get_system_metrics();
//         // Since uptime is very small, requests per minute should be high
//         assert!(metrics.requests_per_minute > 0.0);
//     }
// }
