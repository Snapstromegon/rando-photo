use std::path::PathBuf;

use axum::{
    Extension, Router,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use clap::Parser;
use glob::{MatchOptions, glob_with};
use pathdiff::diff_paths;
use rand::seq::IndexedRandom;
use tokio::signal;
use tower_http::services::ServeDir;
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Parser, Clone)]
struct Args {
    #[clap(long, env = "IMAGES_PATH")]
    pub images_path: PathBuf,
    #[clap(long, env = "FAST_GLOB")]
    pub fast_glob: PathBuf,
    #[clap(long, env = "FINAL_GLOB")]
    pub final_glob: PathBuf,
    #[clap(long, env = "HTTP_ADDRESS", default_value = "0.0.0.0:3000")]
    pub http_address: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    let listener = tokio::net::TcpListener::bind(args.http_address.clone())
        .await
        .unwrap();
    let app = Router::new()
        .route("/random", get(random_image_handler))
        .route("/newest", get(newest_image_handler))
        .nest_service("/images/", ServeDir::new(args.images_path.clone()))
        .layer(Extension(args));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn random_image_handler(Extension(args): Extension<Args>) -> Response {
    let image = random_image(args.images_path.join(args.final_glob));
    if let Some(image) = image {
        println!("Image: {:?}", image);
        Redirect::temporary(&format!(
            "/images/{}",
            diff_paths(image, args.images_path)
                .unwrap()
                .to_string_lossy()
        ))
        .into_response()
    } else {
        println!("No image found");
        (StatusCode::NOT_FOUND, "No image found".to_string()).into_response()
    }
}

async fn newest_image_handler(Extension(args): Extension<Args>) -> Response {
    let image = newest_image(args.images_path.join(args.fast_glob));
    if let Some(image) = image {
        println!("Image: {:?}", image);
        Redirect::temporary(&format!(
            "/images/{}",
            diff_paths(image, args.images_path)
                .unwrap()
                .to_string_lossy()
        ))
        .into_response()
    } else {
        println!("No image found");
        (StatusCode::NOT_FOUND, "No image found".to_string()).into_response()
    }
}

fn random_image(path: PathBuf) -> Option<PathBuf> {
    let all_images = glob_with(
        &path.join("**/*.jpg").to_string_lossy(),
        MatchOptions {
            case_sensitive: false,
            ..Default::default()
        },
    )
    .unwrap();
    let images: Vec<PathBuf> = all_images.map(|x| x.unwrap()).collect();
    images.choose(&mut rand::rng()).map(|i| i.to_path_buf())
}

fn newest_image(path: PathBuf) -> Option<PathBuf> {
    let all_images = glob_with(
        &path.join("**/*.jpg").to_string_lossy(),
        MatchOptions {
            case_sensitive: false,
            ..Default::default()
        },
    )
    .unwrap();
    let images: Vec<PathBuf> = all_images.map(|x| x.unwrap()).collect();
    images
        .iter()
        .max_by_key(|x| x.metadata().unwrap().created().unwrap())
        .map(|i| i.to_path_buf())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
