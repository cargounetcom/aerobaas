mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "aero",
    version,
    about = "AeroBaaS CLI — Agent-Native Backend-as-a-Service"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the AeroBaaS server
    Serve {
        /// Overrides AERO_SERVER_HOST (default 0.0.0.0)
        #[arg(long)]
        host: Option<String>,
        /// Overrides AERO_SERVER_PORT (default 8080)
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Detect a project's framework (upload arrives in Phase 3)
    Deploy {
        #[arg(long)]
        auto_detect: bool,
        #[arg(default_value = ".")]
        path: String,
    },
    /// Create an API key in your active workspace (needs AERO_TOKEN)
    CreateKey {
        #[arg(short, long)]
        name: String,
        #[arg(short, long, num_args = 1.., value_delimiter = ',')]
        scopes: Vec<String>,
        #[arg(long)]
        credits: Option<i64>,
    },
    /// Write a .env with a new JWT secret
    Init {
        #[arg(default_value = ".")]
        path: String,
    },
    /// List deployed functions
    Functions,
    /// List installed skill packs
    Skills,
    /// Scan dependencies for vulnerabilities
    Scan {
        #[arg(default_value = ".")]
        path: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aero=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { host, port } => commands::serve(host, port).await,
        Commands::Deploy { auto_detect, path } => commands::deploy(auto_detect, path).await,
        Commands::CreateKey {
            name,
            scopes,
            credits,
        } => commands::create_key(name, scopes, credits).await,
        Commands::Init { path } => commands::init(path).await,
        Commands::Functions => commands::list_functions().await,
        Commands::Skills => commands::list_skills().await,
        Commands::Scan { path } => commands::scan(path).await,
    }
}
