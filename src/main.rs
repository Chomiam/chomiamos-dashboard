mod api;
mod config;
mod generations;
mod system;
mod updates;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

use crate::api::{create_router, AppState};
use crate::system::SystemCollector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "9090".to_string()).parse::<u16>().unwrap_or(9090);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    println!("╔═══════════════════════════════════════════════════╗");
    println!("║       🎮 ChomiamOS System Dashboard (Rust)        ║");
    println!("║       Catppuccin Mocha • Modern Gaming OS         ║");
    println!("╚═══════════════════════════════════════════════════╝");
    println!("🌐 Dashboard accessible sur : http://{}", addr);

    let collector = Arc::new(Mutex::new(SystemCollector::new()));
    let state = AppState { collector };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = create_router(state).layer(cors);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Optional: automatically open app in browser or app window if requested or not in headless
    let args: Vec<String> = std::env::args().collect();
    let open_browser = args.contains(&"--open".to_string()) || !args.contains(&"--no-open".to_string());

    if open_browser {
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            // Try open app mode with chrome or xdg-open
            let url = format!("http://{}", addr);
            if std::process::Command::new("google-chrome-stable")
                .args([format!("--app={}", url)])
                .spawn()
                .is_err()
            {
                let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
            }
        });
    }

    axum::serve(listener, app).await?;

    Ok(())
}
