use actix_cors::Cors;
use actix_files as fs;
use actix_web::{web, App, HttpServer};
use anyhow::Result;
use simplelink::check_and_generate_admin_token;
use simplelink::{handlers, AppState};
use sqlx::postgres::PgPoolOptions;
use tracing::info;

#[actix_web::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Database connection string from environment
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Create database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(3))
        .connect(&database_url)
        .await?;

    // Run database migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    let admin_token = check_and_generate_admin_token(&pool).await?;

    let state = AppState {
        db: pool,
        admin_token,
    };

    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
    info!("Starting server at http://{}:{}", host, port);

    // Start HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(state.clone()))
            .service(
                web::scope("/api")
                    .route("/shorten", web::post().to(handlers::create_short_url))
                    .route("/links", web::get().to(handlers::get_all_links))
                    .route("/links/{id}", web::delete().to(handlers::delete_link))
                    .route(
                        "/links/{id}/clicks",
                        web::get().to(handlers::get_link_clicks),
                    )
                    .route(
                        "/links/{id}/sources",
                        web::get().to(handlers::get_link_sources),
                    )
                    .route("/auth/register", web::post().to(handlers::register))
                    .route("/auth/login", web::post().to(handlers::login))
                    .route("/health", web::get().to(handlers::health_check)),
            )
            .service(web::resource("/{short_code}").route(web::get().to(handlers::redirect_to_url)))
            .service(fs::Files::new("/", "./static").index_file("index.html"))
    })
    .workers(2)
    .backlog(10_000)
    .bind(format!("{}:{}", host, port))?
    .run()
    .await?;

    Ok(())
}
