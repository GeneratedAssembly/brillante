use std::path::PathBuf;

use brillante::app;
use brillante::config::AppConfig;

struct Args {
    config: Option<PathBuf>,
    host: Option<String>,
    port: Option<u16>,
    courses: Vec<PathBuf>,
}

const USAGE: &str = "brillante — interactive course platform

USAGE:
    brillante [OPTIONS]

OPTIONS:
    -c, --config <path>    config file to load (default: brillante.toml if present)
        --host <addr>      bind address (default: 127.0.0.1, overrides config)
    -p, --port <port>      bind port (default: 3000, overrides config)
        --course <dir>     register an extra course directory (repeatable)
    -h, --help             print this help
";

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        config: None,
        host: None,
        port: None,
        courses: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            "-c" | "--config" => {
                args.config = Some(
                    it.next()
                        .ok_or_else(|| format!("{a} expects a path"))?
                        .into(),
                );
            }
            "--host" => {
                args.host = Some(it.next().ok_or_else(|| format!("{a} expects a value"))?);
            }
            "-p" | "--port" => {
                args.port = Some(
                    it.next()
                        .ok_or_else(|| format!("{a} expects a port"))?
                        .parse()
                        .map_err(|_| format!("{a} expects a numeric port"))?,
                );
            }
            "--course" => {
                let dir: PathBuf = it
                    .next()
                    .ok_or_else(|| format!("{a} expects a directory"))?
                    .into();
                args.courses.push(dir);
            }
            other => return Err(format!("unknown argument '{other}'. try --help")),
        }
    }
    Ok(args)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("brillante=info,info")),
        )
        .init();

    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    let config_path = args.config.clone().or_else(|| {
        PathBuf::from("brillante.toml").is_file().then_some(PathBuf::from("brillante.toml"))
    });

    let app_config = match AppConfig::load(
        config_path.as_deref(),
        &args.courses,
        args.host.clone(),
        args.port,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    for w in &app_config.warnings {
        tracing::warn!("{w}");
    }
    if app_config.courses.is_empty() {
        eprintln!(
            "error: no course directories registered. create brillante.toml with \
             courses = [\"content/my-course\"] or pass --course <dir>"
        );
        std::process::exit(1);
    }

    app::load_bootstrap_env();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    runtime.block_on(serve(app_config));
}

async fn serve(config: AppConfig) {
    let router = app::router(&config);
    app::spawn_admin_bootstrap(&config);
    let addr = format!("{}:{}", config.host, config.port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: cannot bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("brillante serving on http://{addr}");
    for course in &config.courses {
        tracing::info!("course directory: {}", course.display());
    }
    match &config.pocketbase {
        Some(pb) => tracing::info!("pocketbase auth: {} (accounts at /account)", pb.url),
        None => tracing::info!("pocketbase auth: disabled (no [pocketbase] section)"),
    }
    tracing::info!("reload content: POST /admin/reload · diagnostics: GET /admin/diagnostics");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
