use anyhow::Result;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::config::Config;
use crate::routes;

/// Binds to the configured address and drives the server until shutdown.
///
/// # Errors
///
/// Returns an error if the TCP listener cannot bind or if axum returns
/// an IO error during serving.
pub async fn run(config: Config) -> Result<()> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::build()
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(cors);

    let addr: std::net::SocketAddr = config.listen_addr.parse()?;
    tracing::info!(addr = %addr, "control-api listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
