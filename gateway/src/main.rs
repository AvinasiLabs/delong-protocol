use axum::{
    Router,
    response::Json,
    routing::{get, post},
    serve,
};
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::Resource;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tracing::{info, instrument};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
fn init_tracing() -> anyhow::Result<()> {
    let trace_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()?;

    let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(trace_exporter)
        .with_resource(
            Resource::builder()
                .with_service_name("delong-gateway")
                .build(),
        )
        .build();

    let tracer = tracer_provider.tracer("delong-gateway");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let filter = EnvFilter::new("gateway=trace") // Only capture traces from your gateway crate
        .add_directive("tower=off".parse()?) // Turn off tower spans
        .add_directive("hyper=off".parse()?) // Turn off hyper spans
        .add_directive("h2=off".parse()?) // Turn off h2 spans
        .add_directive("tonic=off".parse()?) // Turn off tonic spans
        .add_directive("opentelemetry=off".parse()?) // Turn off opentelemetry internal spans
        .add_directive("axum=off".parse()?) // Turn off axum spans
        .add_directive("tokio=off".parse()?) // Turn off tokio spans
        .add_directive("runtime=off".parse()?); // Turn off runtime spans

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().json()) // Keep console output for debugging
        .with(telemetry) // OpenTelemetry traces
        .with(filter)
        .init();

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing()?;

    let app = Router::new()
        .route("/health", get(health))
        .route("/echo", post(echo));

    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await?;
    info!("listening on {}", addr);

    serve(listener, app).await?;
    Ok(())
}

#[instrument]
async fn health() -> Json<Value> {
    info!("health check");
    Json(json!({"status": "healthy"}))
}

#[derive(Deserialize, Serialize, Debug)]
struct Echo {
    msg: String,
}

#[instrument]
async fn echo(Json(payload): Json<Echo>) -> Json<Echo> {
    info!("echoing: {}", payload.msg);
    Json(payload)
}
