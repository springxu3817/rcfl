use anyhow::Result;
use axum::extract::State;
use axum::{http::StatusCode, routing::get, Router};
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name="rcli", version, author, about, long_about= None)]
struct Opts {
    #[command(subcommand)]
    cmd: SubCommand,
}

#[derive(Debug, Subcommand)]
enum SubCommand {
    #[command(subcommand)]
    Http(HttpSubCommand),
}

#[derive(Debug, Subcommand)]
enum HttpSubCommand {
    Serve(HttpServeOpts),
}

#[derive(Debug, Parser)]
struct HttpServeOpts {
    #[arg(short, long, value_parser=verify_path, default_value=".")]
    dir: PathBuf,

    #[arg(short, long, default_value_t = 8080)]
    port: u16,
}

#[derive(Debug)]
struct HttpServeState {
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let opts = Opts::parse();
    println!("test {:?}", opts);
    match opts.cmd {
        SubCommand::Http(s) => match s {
            HttpSubCommand::Serve(opts) => {
                process_http_serve(opts.dir, opts.port).await?;
            }
        },
    }
    Ok(())
}

fn verify_path(path: &str) -> Result<PathBuf, &'static str> {
    // if input is "-" or file exists
    let p = Path::new(path);
    if p.exists() && p.is_dir() {
        Ok(path.into())
    } else {
        Err("Path does not exist or is not a directory")
    }
}

async fn process_http_serve(path: PathBuf, port: u16) -> Result<()> {
    info!("Serving {:?} on port {}", path, port);
    let state = HttpServeState { path };
    let router = Router::new()
        .route("/*reqPath", get(file_handler))
        .with_state(Arc::new(state));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, router).await?;
    Ok(())
}

async fn file_handler(
    State(state): State<Arc<HttpServeState>>,
    axum::extract::Path(req_path): axum::extract::Path<String>,
) -> (StatusCode, String) {
    let p: PathBuf = std::path::Path::new(&state.path).join(req_path);
    if !p.exists() {
        (StatusCode::NOT_FOUND, format!("File {:?} not found!", p))
    } else {
        match tokio::fs::read_to_string(p).await {
            Ok(content) => (StatusCode::OK, content),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        }
    }
}
