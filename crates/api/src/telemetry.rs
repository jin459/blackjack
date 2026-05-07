use anyhow::Context;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime::Tokio, trace as sdktrace, Resource};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialise OTel tracer + tracing-subscriber (JSON logs + OTel spans).
/// Call once at startup before any async work begins.
pub fn init(service_name: &str, otlp_endpoint: &str) -> anyhow::Result<()> {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(otlp_endpoint),
        )
        .with_trace_config(
            sdktrace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                service_name.to_string(),
            )])),
        )
        .install_batch(Tokio)
        .context("failed to install OTel tracer")?;

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().json().flatten_event(true))
        .with(OpenTelemetryLayer::new(tracer))
        .init();

    Ok(())
}

/// Flush buffered spans — call on graceful shutdown.
pub fn shutdown() {
    global::shutdown_tracer_provider();
}
