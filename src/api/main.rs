use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode, Uri},
    response::{Json, Response},
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::error::Error as StdError;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{info, warn};

mod middleware;
mod models;
mod routes;
mod services;

// DrawIO and Export modules are at crate root - add them to binary's module tree
#[path = "../drawio/mod.rs"]
mod drawio;

#[path = "../export/mod.rs"]
mod export;

use routes::create_api_router;

// Panic hook to catch and log panics
fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("PANIC occurred!");
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("Panic message: {}", s);
        }
        if let Some(location) = panic_info.location() {
            eprintln!(
                "Panic location: {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }
        eprintln!("Backtrace:\n{:?}", std::backtrace::Backtrace::capture());
    }));
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn StdError + Send + Sync + 'static>> {
    // Setup panic hook first
    setup_panic_hook();

    // Force stdout/stderr to be line-buffered for Docker
    use std::io::Write;
    std::io::stdout().flush().ok();
    std::io::stderr().flush().ok();
    eprintln!("[1] Starting application...");

    // Initialize tracing with debug-level support
    // RUST_LOG environment variable controls log level (default: info)
    eprintln!("[2] Setting up tracing...");
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        eprintln!("[2a] Using default log level: info");
        tracing_subscriber::EnvFilter::new("info")
    });

    // Initialize tracing subscriber with OpenTelemetry support
    // Write to stderr (which will be captured by the test script)
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .with_ansi(false) // Disable ANSI colors for log files
        .init();

    eprintln!("[4] Tracing initialized");
    info!("Application starting...");

    // Initialize OpenTelemetry observability with OTLP exporter
    // This sets up metrics and distributed tracing
    if let Err(e) = middleware::observability::init_observability().await {
        warn!(
            "Failed to initialize observability: {}. Continuing without OTLP export.",
            e
        );
    }

    // Build application with routes
    eprintln!("[5] API router will be created with app state below...");

    // Determine frontend static files directory
    // In production: frontend-dioxus/dist (built Dioxus app) or frontend-react/dist (React app)
    // In development: can be empty (frontend served separately)
    let frontend_dir = std::env::var("FRONTEND_DIR")
        .unwrap_or_else(|_| "frontend-dioxus/dist".to_string());
    let frontend_path = PathBuf::from(&frontend_dir);
    
    // Determine WASM files directory (relative to frontend dist or separate)
    // WASM files can be in frontend/dist/wasm (after build) or frontend/public/wasm (source)
    let wasm_dir = std::env::var("WASM_DIR")
        .unwrap_or_else(|_| {
            // Check if wasm exists in dist first, then fallback to public
            let dist_wasm = format!("{}/wasm", frontend_dir);
            let dist_wasm_path = PathBuf::from(&dist_wasm);
            if dist_wasm_path.exists() {
                dist_wasm
            } else if frontend_dir != "frontend/dist" {
                format!("{}/wasm", frontend_dir)
            } else {
                "frontend/public/wasm".to_string()
            }
        });
    let wasm_path = PathBuf::from(&wasm_dir);

    eprintln!("[6] Frontend directory: {:?}, exists: {}", frontend_path, frontend_path.exists());
    eprintln!("[7] WASM directory: {:?}, exists: {}", wasm_path, wasm_path.exists());

    // Build main app - apply with_state FIRST, then layers
    // This ensures router becomes Router<()> before middleware is applied
    eprintln!("[8] Building main app router...");
    let app_state = routes::create_app_state();

    // Build the main router: health checks + API routes nested under /api/v1
    // Nest the API router (with AppState) first, then add other routes
    let mut app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/health", get(health_check))
        .nest("/api/v1", routes::create_api_router(app_state.clone()));

    // Add static file serving for frontend and WASM if directories exist
    if frontend_path.exists() {
        info!("Serving frontend from: {:?}", frontend_path);
        eprintln!("[8a] Frontend path exists, setting up static file serving");
        
        // Serve WASM files at root (/) so they're accessible at /data-modeller/... etc.
        // Files in wasm/data-modeller/ will be accessible at /data-modeller/
        // This must come after API routes but before frontend assets to avoid conflicts
        if wasm_path.exists() {
            info!("Serving WASM files from: {:?} at root (/)", wasm_path);
            eprintln!("[8b] WASM path exists, setting up WASM serving at root");
            // Serve each subdirectory of WASM at root level
            // This allows /data-modeller/... to work directly
            if let Ok(entries) = std::fs::read_dir(&wasm_path) {
                let mut wasm_dirs_found = 0;
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            let dir_name = entry.file_name();
                            let dir_name_str = dir_name.to_string_lossy();
                            let dir_path = entry.path();
                            let route_path = format!("/{}", dir_name_str);
                            wasm_dirs_found += 1;
                            info!("Serving WASM subdirectory {:?} at {}", dir_path, route_path);
                            eprintln!("[8c] Found WASM subdirectory: {} -> {}", route_path, dir_path.display());
                            app = app.nest_service(
                                route_path.as_str(),
                                ServeDir::new(&dir_path)
                                    .append_index_html_on_directories(false)
                                    .precompressed_gzip()
                                    .precompressed_br(),
                            );
                        }
                    }
                }
                eprintln!("[8d] Total WASM subdirectories found: {}", wasm_dirs_found);
            } else {
                eprintln!("[8e] Failed to read WASM directory: {:?}", wasm_path);
            }
        } else {
            eprintln!("[8f] WASM path does not exist: {:?}", wasm_path);
        }

        // Serve frontend static files (JS, CSS, images, etc.)
        // This must come before the SPA fallback route
        app = app.nest_service(
            "/assets",
            ServeDir::new(&frontend_path.join("assets"))
                .append_index_html_on_directories(false)
                .precompressed_gzip()
                .precompressed_br(),
        );

        // Serve other static files from frontend dist root (favicon, etc.)
        // This fallback should come last to serve index.html for SPA routes
        app = app.fallback_service(
            ServeDir::new(&frontend_path)
                .append_index_html_on_directories(true)
                .precompressed_gzip()
                .precompressed_br(),
        );
    } else {
        warn!("Frontend directory not found: {:?}. Frontend will not be served.", frontend_path);
        warn!("Set FRONTEND_DIR environment variable to enable frontend serving.");
        // Add SPA fallback for when frontend is served separately (dev mode)
        app = app.fallback(serve_spa_fallback);
    }

    let app = app.with_state(app_state);

    // Apply middleware layers
    let app = app.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive()),
    );
    eprintln!("[9] App router built with state and middleware");

    // Run server on configurable port (default 8081 for API)
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081);
    eprintln!("[12] Setting up server address...");
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Server listening on {} (port {})", addr, port);
    info!("Health check available at http://{}/health", addr);
    info!(
        "API health check available at http://{}/api/v1/health",
        addr
    );
    if frontend_path.exists() {
        info!("Frontend available at http://{}", addr);
    } else {
        info!("Frontend not served (set FRONTEND_DIR to enable)");
    }

    eprintln!("[13] Binding TCP listener to {}...", addr);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            eprintln!("[14] TCP listener bound successfully");
            l
        }
        Err(e) => {
            eprintln!("[14] ERROR: Failed to bind listener: {}", e);
            // std::io::Error implements all required traits, convert to expected type
            let err: Box<dyn StdError + Send + Sync + 'static> = Box::new(e);
            return Err(err);
        }
    };

    // Setup graceful shutdown
    // Handle both SIGINT (Ctrl+C) and SIGTERM (Docker stop)
    // Note: Graceful shutdown is currently disabled (commented out below)
    #[cfg(unix)]
    let _shutdown_signal = async {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm =
            signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("SIGINT received, shutting down gracefully");
            }
            _ = sigterm.recv() => {
                info!("SIGTERM received, shutting down gracefully");
            }
        }
    };

    #[cfg(not(unix))]
    let _shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Shutdown signal received");
    };

    // Run server with graceful shutdown
    // In axum 0.8, Router with state implements IntoMakeService automatically
    // axum::serve accepts Router directly and handles the conversion internally
    eprintln!("[15] Starting axum server...");
    info!("Starting server...");

    // In axum 0.8, Router with state can be used directly with axum::serve
    // The router automatically implements the required traits
    eprintln!("[10] Starting axum server...");
    info!("Server starting, listening on {}", addr);
    eprintln!("[11] About to call axum::serve...");
    // Temporarily without graceful shutdown to debug
    // axum::serve should block indefinitely, so if it returns, something went wrong
    // axum::serve returns Result<(), std::io::Error>
    // std::io::Error implements std::error::Error + Send + Sync + 'static
    // Convert it to our error type
    match axum::serve(listener, app).await {
        Ok(()) => {
            eprintln!("[12] axum::serve returned (unexpected - should block forever)");
        }
        Err(e) => {
            eprintln!("[12] ERROR: axum::serve returned error: {}", e);
            // std::io::Error implements all required traits, convert to expected type
            let err: Box<dyn StdError + Send + Sync + 'static> = Box::new(e);
            return Err(err);
        }
    }
    eprintln!("[12] axum::serve returned (unexpected - should block forever)");

    // With graceful shutdown (re-enable after debugging)
    // axum::serve(listener, app)
    //     .with_graceful_shutdown(shutdown_signal)
    //     .await?;

    eprintln!("[18] Server shutdown complete");
    info!("Server shutdown complete");

    // Shutdown observability
    middleware::observability::shutdown_observability().await;

    Ok(())
}

async fn health_check(_state: axum::extract::State<routes::tables::AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "modelling-api",
        "version": "0.1.0"
    }))
}

/// SPA fallback handler - serves index.html for non-API routes
/// Used when frontend is served separately (e.g., Vite dev server)
async fn serve_spa_fallback(uri: Uri) -> Result<Response<Body>, StatusCode> {
    // Only serve index.html for non-API routes
    if uri.path().starts_with("/api") {
        return Err(StatusCode::NOT_FOUND);
    }

    // In dev mode, redirect to Vite dev server or return a helpful message
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Data Modelling API</title>
    <style>
        body { font-family: sans-serif; padding: 2rem; max-width: 800px; margin: 0 auto; }
        .info { background: #e3f2fd; padding: 1rem; border-radius: 4px; margin: 1rem 0; }
        .code { background: #f5f5f5; padding: 0.5rem; border-radius: 4px; font-family: monospace; }
    </style>
</head>
<body>
    <h1>Data Modelling API</h1>
    <p>The API is running, but the frontend is not being served by this server.</p>
    <div class="info">
        <strong>Development Mode:</strong> Start the frontend dev server:
        <div class="code">cd frontend && npm run dev</div>
        <p>Then access the app at <a href="http://localhost:5173">http://localhost:5173</a></p>
    </div>
    <div class="info">
        <strong>Production Mode:</strong> Set the FRONTEND_DIR environment variable to the built frontend directory:
        <div class="code">FRONTEND_DIR=frontend/dist</div>
    </div>
    <h2>API Endpoints</h2>
    <ul>
        <li><a href="/api/v1/health">/api/v1/health</a> - Health check</li>
        <li><a href="/health">/health</a> - Health check (alternative)</li>
    </ul>
</body>
</html>
    "#;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
        .body(Body::from(html))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
}
