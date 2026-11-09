pub mod api;
pub mod web;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::response::Html;
use axum::routing::{get, post};
use tower_http::cors::{Any, CorsLayer};

pub fn build_router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Embedded Web App
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        // REST API
        .route("/api/status", get(api::status_handler))
        .route("/api/search", get(api::search_handler))
        .route("/api/detail", get(api::detail_handler))
        .route("/api/export", post(api::export_handler))
        .route("/api/clean-step", post(api::clean_step_handler))
        // Support larger STEP uploads (up to 128MB)
        .layer(DefaultBodyLimit::max(128 * 1024 * 1024))
        .layer(cors)
}

async fn index_handler() -> Html<&'static str> {
    Html(web::INDEX_HTML)
}

/// Run the SeEx HTTP Web Server
pub fn run_server(
    host: String,
    port: u16,
    open_browser: bool,
    _output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    api::init_server_start_time();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let app = build_router();
        let addr: SocketAddr = format!("{}:{}", host, port)
            .parse()
            .map_err(|e| format!("Invalid host/port '{}:{}': {}", host, port, e))?;

        println!("============================================================");
        println!("🚀 SeEx EDA Web Server running on: http://{}", addr);
        println!(
            "🌐 Access Web UI via your browser: http://{}:{}",
            if host == "0.0.0.0" {
                "localhost"
            } else {
                &host
            },
            port
        );
        println!("📡 API Endpoints:");
        println!("   - GET  /api/status");
        println!("   - GET  /api/search?q=<KEYWORD_OR_ID>");
        println!("   - GET  /api/detail?id=<LCSC_ID>");
        println!("   - POST /api/export");
        println!("   - POST /api/clean-step?mode=logo_only&filename=model.step");
        println!("============================================================");

        if open_browser {
            let browser_url = format!(
                "http://{}:{}",
                if host == "0.0.0.0" {
                    "127.0.0.1"
                } else {
                    &host
                },
                port
            );
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", &browser_url])
                    .spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open").arg(&browser_url).spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&browser_url)
                    .spawn();
            }
        }

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok::<(), Box<dyn std::error::Error>>(())
    })?;

    Ok(())
}
