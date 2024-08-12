use anyhow::Result;
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::{http::StatusCode, routing::get, Router};
use axum_macros::debug_handler;
use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::{
    io::Error,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::info;
use urlencoding::encode;

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
    println!("test change {:?}", opts);
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

// Traverse directorie
async fn get_cur_dir_files(dir: &PathBuf) -> Result<Vec<OsString>, std::io::Error> {
    let mut files = vec![];
    files.clear();
    if dir.is_dir() {
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            files.push(entry.file_name());
            //println!("{:?} {:?}", entry.path(), entry.file_name());
        }
        //println!("vec  {:?}", files);
        Ok(files)
    } else {
        Err(Error::from(std::io::ErrorKind::InvalidData))
    }
}

#[debug_handler]
async fn file_handler(
    State(state): State<Arc<HttpServeState>>,
    axum::extract::Path(req_path): axum::extract::Path<String>,
) -> (StatusCode, axum::response::Response) {
    let pathbuf: PathBuf = std::path::Path::new(&state.path).join(req_path.clone());
    println!("file_handler DirPath {:?}", pathbuf);
    if !pathbuf.exists() {
        // not found return code = 404
        (
            StatusCode::NOT_FOUND,
            format!("File {} not found!", req_path).into_response(),
        )
    } else if pathbuf.is_dir() {
        let mut str_list: String = String::new();
        let res = get_cur_dir_files(&pathbuf).await;
        match res {
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string().into_response(),
                );
            }
            Ok(content) => {
                str_list
                    .push_str(format!("<li><a href={}>{}</li> ", encode(&req_path), ".").as_str());
                for filename in content {
                    let str_file_name = filename.into_string();
                    match str_file_name {
                        Ok(res_file_name) => {
                            let filepath = format!("{}/{}", req_path, res_file_name);
                            str_list.push_str(
                                format!(
                                    "<li><a href={}>{:?}</li> ",
                                    encode(&filepath),
                                    res_file_name
                                )
                                .as_str(),
                            );
                        }
                        Err(str_os_file_name) => {
                            str_list.push_str(format!("<li>{:?}</li> ", str_os_file_name).as_str());
                        }
                    }
                }
            }
        }
        let mut str_body = String::new();
        str_body.push_str(format!("<html><body>{}</body></html>", str_list).as_str());
        return (
            StatusCode::OK,
            Html::<String>::from(str_body).into_response(),
        );
    } else {
        match tokio::fs::read_to_string(&pathbuf).await {
            //read utf8 txt
            Ok(content) => (StatusCode::OK, content.into_response()), //OK 200
            Err(e) => match e.kind() {
                std::io::ErrorKind::InvalidData => match tokio::fs::read(&pathbuf).await {
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
