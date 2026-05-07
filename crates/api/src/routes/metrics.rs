use actix_web::{get, web, HttpResponse};
use prometheus::{Encoder, Registry, TextEncoder};

/// `GET /metrics` — Prometheus scrape endpoint.
#[get("/metrics")]
pub async fn prometheus_metrics(registry: web::Data<Registry>) -> HttpResponse {
    let encoder = TextEncoder::new();
    let families = registry.gather();
    let mut buf = Vec::new();

    if let Err(e) = encoder.encode(&families, &mut buf) {
        tracing::error!(err = %e, "failed to encode metrics");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8; version=0.0.4")
        .body(buf)
}
