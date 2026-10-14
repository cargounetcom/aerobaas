use aero_config::AeroConfig;
use aero_skills::FrameworkDetector;

/// Server URL for client commands (`AERO_URL`, default http://localhost:8080).
fn base_url() -> String {
    std::env::var("AERO_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "http://localhost:8080".into())
        .trim_end_matches('/')
        .to_string()
}

/// Client with `Authorization: Bearer $AERO_TOKEN` (a sign-in token or an `aero_…` API key).
fn client() -> anyhow::Result<(reqwest::Client, String)> {
    let token = std::env::var("AERO_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Set AERO_TOKEN to an API key (aero_…) or a sign-in token, and AERO_URL if the server is not on localhost:8080"
            )
        })?;
    Ok((reqwest::Client::new(), format!("Bearer {}", token.trim())))
}

async fn print_response(r: reqwest::Response) -> anyhow::Result<()> {
    let status = r.status();
    let body: serde_json::Value = r.json().await.unwrap_or(serde_json::Value::Null);
    println!("{body:#}");
    if !status.is_success() {
        anyhow::bail!("server answered {status}");
    }
    Ok(())
}

fn unreachable(e: reqwest::Error) -> anyhow::Error {
    anyhow::anyhow!(
        "cannot reach {} ({e}). Start the server with: aero serve",
        base_url()
    )
}

pub async fn serve(host: Option<String>, port: Option<u16>) -> anyhow::Result<()> {
    let mut config = AeroConfig::from_env();
    if let Some(h) = host {
        config.server.host = h;
    }
    if let Some(p) = port {
        config.server.port = p;
    }
    config.validate().map_err(anyhow::Error::msg)?;

    let app = aero_core::AeroApp::new(config).await?;
    std::sync::Arc::new(app).serve().await
}

pub async fn deploy(auto_detect: bool, path: String) -> anyhow::Result<()> {
    if !auto_detect {
        println!("Use --auto-detect to detect the framework");
        return Ok(());
    }
    let framework = FrameworkDetector::detect(&path);
    println!("Detected framework: {framework}");
    println!("Uploading builds from the CLI arrives in Phase 3 (see ROADMAP.md). Deploy a .wasm module with POST /v1/functions.");
    Ok(())
}

fn parse_scope(s: &str) -> Option<aero_protocol::keys::Scope> {
    use aero_protocol::keys::Scope::*;
    Some(match s.trim().replace('_', ".").as_str() {
        "functions.read" => FunctionsRead,
        "functions.write" => FunctionsWrite,
        "functions.list" => FunctionsList,
        "executions.write" => ExecutionsWrite,
        "executions.read" => ExecutionsRead,
        "databases.read" => DatabasesRead,
        "databases.write" => DatabasesWrite,
        "ai.inference" => AiInference,
        "skills.manage" => SkillsManage,
        "remediation.run" => RemediationRun,
        "mcp.connect" => McpConnect,
        "admin" => Admin,
        _ => return None,
    })
}

pub async fn create_key(
    name: String,
    scopes: Vec<String>,
    credits: Option<i64>,
) -> anyhow::Result<()> {
    let mut parsed = Vec::new();
    for s in &scopes {
        parsed.push(parse_scope(s).ok_or_else(|| anyhow::anyhow!("unknown scope `{s}`"))?);
    }
    let req = aero_protocol::keys::CreateKeyRequest {
        name,
        scopes: parsed,
        allowed_function_ids: None,
        allowed_database_ids: None,
        credits_limit: credits,
        expires_at: None,
    };
    let (http, auth) = client()?;
    let r = http
        .post(format!("{}/v1/keys", base_url()))
        .header("authorization", auth)
        .json(&req)
        .send()
        .await
        .map_err(unreachable)?;
    print_response(r).await
}

/// Write a `.env` with a freshly generated JWT secret.
pub async fn init(path: String) -> anyhow::Result<()> {
    let env_path = std::path::Path::new(&path).join(".env");
    if env_path.exists() {
        anyhow::bail!("{} already exists; not overwriting it", env_path.display());
    }
    let secret = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let content = format!(
        "# AeroBaaS configuration — see .env.example in the AeroBaaS repository\n\
         AERO_JWT_SECRET={secret}\n\
         AERO_DATABASE_URL=postgres://aero:CHANGE_PASSWORD@localhost:5432/aero\n\
         AERO_PUBLIC_URL=http://localhost:8080\n"
    );
    std::fs::write(&env_path, content)?;
    println!(
        "Wrote {} with a new AERO_JWT_SECRET. Set AERO_DATABASE_URL, then run: aero serve",
        env_path.display()
    );
    Ok(())
}

pub async fn list_functions() -> anyhow::Result<()> {
    let (http, auth) = client()?;
    let r = http
        .get(format!("{}/v1/functions", base_url()))
        .header("authorization", auth)
        .send()
        .await
        .map_err(unreachable)?;
    print_response(r).await
}

pub async fn list_skills() -> anyhow::Result<()> {
    let (http, auth) = client()?;
    let r = http
        .get(format!("{}/v1/skills", base_url()))
        .header("authorization", auth)
        .send()
        .await
        .map_err(unreachable)?;
    print_response(r).await
}

pub async fn scan(path: String) -> anyhow::Result<()> {
    let pkg_json = std::path::Path::new(&path).join("package.json");
    let cargo_toml = std::path::Path::new(&path).join("Cargo.toml");

    let (manifest_type, manifest_content) = if pkg_json.exists() {
        ("package_json", std::fs::read_to_string(&pkg_json)?)
    } else if cargo_toml.exists() {
        ("cargo_toml", std::fs::read_to_string(&cargo_toml)?)
    } else {
        anyhow::bail!("No package.json or Cargo.toml found in {path}");
    };
    let (http, auth) = client()?;
    let r = http
        .post(format!("{}/v1/remediation/scan", base_url()))
        .header("authorization", auth)
        .json(&serde_json::json!({
            "manifest_type": manifest_type,
            "manifest_content": manifest_content
        }))
        .send()
        .await
        .map_err(unreachable)?;
    print_response(r).await
}
