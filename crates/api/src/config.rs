use anyhow::Context;
use config::{Config, Environment};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_host")]
    pub server_host: String,

    #[serde(default = "default_port")]
    pub server_port: u16,

    pub redis_url: String,

    #[serde(default = "default_otlp")]
    pub otel_exporter_otlp_endpoint: String,

    #[serde(default = "default_service_name")]
    pub otel_service_name: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Config::builder()
            .set_default("server_host", default_host())?
            .set_default("server_port", default_port() as i64)?
            .set_default("redis_url", "redis://127.0.0.1:6379")?
            .set_default("otel_exporter_otlp_endpoint", default_otlp())?
            .set_default("otel_service_name", default_service_name())?
            .add_source(Environment::default())
            .build()
            .context("failed to build config")?
            .try_deserialize()
            .context("failed to deserialize config")
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}

fn default_host() -> String {
    "0.0.0.0".into()
}
fn default_port() -> u16 {
    8080
}
fn default_otlp() -> String {
    "http://localhost:4317".into()
}
fn default_service_name() -> String {
    "blackjack-api".into()
}
