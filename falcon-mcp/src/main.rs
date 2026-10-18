use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "falcon-mcp",
    version,
    about = "MCP server for the falcon-detective coding agent"
)]
struct Args {
    /// Sandbox root: every fs/cargo/git path resolves inside this dir.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Use stdio transport (default). Mutually exclusive with --http.
    #[arg(long, group = "transport")]
    stdio: bool,

    /// Use HTTP transport on the given port.
    #[arg(long, group = "transport")]
    http: Option<u16>,

    /// Disable all writes (read-only mode).
    #[arg(long)]
    read_only: bool,

    /// Enable the exec.run tool (off by default for safety).
    #[arg(long)]
    enable_exec: bool,

    /// Extra honeytoken file name to watch, on top of CONFIDENTIAL_KEYS.txt.
    /// Addressing one fails the call and revokes the session. Repeatable.
    #[arg(long = "honeytoken", value_name = "NAME")]
    honeytokens: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr) // never write logs to stdout in stdio mode
        .init();

    let args = Args::parse();

    let sandbox = falcon_mcp::Sandbox::new(args.root.clone(), args.read_only)?
        .with_honeytokens(args.honeytokens.iter().cloned());
    let server = falcon_mcp::FalconMcp::new_with_options(sandbox, args.enable_exec);

    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    };
    use rmcp::ServiceExt;

    if let Some(port) = args.http {
        let service = StreamableHttpService::new(
            // Fresh revocation flag per session: a tripwire revokes only
            // the session that tripped it.
            move || Ok(server.for_new_session()),
            LocalSessionManager::default().into(),
            Default::default(),
        );
        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
        tracing::info!("falcon-mcp HTTP listening on :{port}");
        axum::serve(listener, router).await?;
    } else {
        let svc = server.serve(rmcp::transport::stdio()).await?;
        svc.waiting().await?;
    }
    Ok(())
}
