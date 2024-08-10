use anyhow::Result;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::{http::StatusCode, routing::get, Router};
use axum_macros::debug_handler;
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

#[debug_handler]
async fn file_handler(
    State(state): State<Arc<HttpServeState>>,
    axum::extract::Path(req_path): axum::extract::Path<String>,
) -> (StatusCode, axum::response::Response) {
    let p: PathBuf = std::path::Path::new(&state.path).join(req_path);
    if !p.exists() {
        // not found return code = 404
        (
            StatusCode::NOT_FOUND,
            format!("File {:?} not found!", p).into_response(),
        )
    } else {
        match tokio::fs::read_to_string(&p).await {
            //read utf8 txt
            Ok(content) => (StatusCode::OK, content.into_response()), //OK 200
            Err(e) => match e.kind() {
                std::io::ErrorKind::InvalidData => match tokio::fs::read(&p).await {
                    //read octet-stream
                    Ok(byte_array) => (StatusCode::OK, byte_array.into_response()),
                    //read octet-stream error
                    Err(e1) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        e1.to_string().into_response(),
                    ),
                },
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string().into_response(),
                ),
            },
        }
    }
}
