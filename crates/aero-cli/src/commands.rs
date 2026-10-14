use aero_config::AeroConfig;
use aero_crypto::generate_api_key;
use aero_skills::FrameworkDetector;

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
    if auto_detect {
        let framework = FrameworkDetector::detect(&path);
        println!("Detected framework: {framework}");

        let client = reqwest::Client::new();
        let resp = client
            .post("http://localhost:8080/v1/deploy/auto-detect")
            .json(&serde_json::json!({ "workspace_path": path }))
            .send()
            .await;

        match resp {
            Ok(r) => {
                let body: serde_json::Value = r.json().await?;
                println!("Deploy result: {body}");
            }
            Err(_) => {
                println!("Server not running. Would deploy {framework} from {path}");
            }
        }
    } else {
        println!("Use --auto-detect to deploy");
    }
    Ok(())
}

pub async fn create_key(
    name: String,
    scopes: Vec<String>,
    credits: Option<i64>,
) -> anyhow::Result<()> {
    let scope_enums: Vec<aero_protocol::keys::Scope> = scopes
        .iter()
        .filter_map(|s| match s.as_str() {
            "functions.read" => Some(aero_protocol::keys::Scope::FunctionsRead),
            "functions.write" => Some(aero_protocol::keys::Scope::FunctionsWrite),
            "functions.list" => Some(aero_protocol::keys::Scope::FunctionsList),
            "executions.write" => Some(aero_protocol::keys::Scope::ExecutionsWrite),
            "executions.read" => Some(aero_protocol::keys::Scope::ExecutionsRead),
            "databases.read" => Some(aero_protocol::keys::Scope::DatabasesRead),
            "databases.write" => Some(aero_protocol::keys::Scope::DatabasesWrite),
            "ai.inference" => Some(aero_protocol::keys::Scope::AiInference),
            "skills.manage" => Some(aero_protocol::keys::Scope::SkillsManage),
            "remediation.run" => Some(aero_protocol::keys::Scope::RemediationRun),
            "mcp.connect" => Some(aero_protocol::keys::Scope::McpConnect),
            "admin" => Some(aero_protocol::keys::Scope::Admin),
            _ => None,
        })
        .collect();

    let req = aero_protocol::keys::CreateKeyRequest {
        name,
        scopes: scope_enums,
        allowed_function_ids: None,
        allowed_database_ids: None,
        credits_limit: credits,
        expires_at: None,
    };

    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:8080/v1/keys")
        .json(&req)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let body: serde_json::Value = r.json().await?;
            println!("Key created: {body}");
        }
        Err(_) => {
            let key = generate_api_key();
            println!("Server not running. Generated key: {key}");
        }
    }
    Ok(())
}

pub async fn init(path: String) -> anyhow::Result<()> {
    let config = AeroConfig::default();
    let config_path = std::path::Path::new(&path).join("aero.toml");
    let config_content = format!(
        r#"[server]
host = "{}"
port = {}

[database]
url = "{}"
namespace = "{}"

[wasm]
max_memory_mb = {}
hazmat_egress_filtering = true

[ai]
model_name = "{}"
quantization = "{}"

[security]
hazmat_enabled = true
remediation_enabled = true
"#,
        config.server.host,
        config.server.port,
        config.database.url,
        config.database.namespace,
        config.wasm.max_memory_mb,
        config.ai.model_name,
        config.ai.quantization
    );

    std::fs::write(&config_path, config_content)?;
    println!("Initialized AeroBaaS project at {path}/aero.toml");
    Ok(())
}

pub async fn list_functions() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    match client
        .get("http://localhost:8080/v1/functions")
        .send()
        .await
    {
        Ok(r) => {
            let body: serde_json::Value = r.json().await?;
            println!("{body:#}");
        }
        Err(_) => println!("Server not running. Start with: aero serve"),
    }
    Ok(())
}

pub async fn list_skills() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    match client.get("http://localhost:8080/v1/skills").send().await {
        Ok(r) => {
            let body: serde_json::Value = r.json().await?;
            println!("{body:#}");
        }
        Err(_) => {
            println!("Built-in skill packs:");
            println!("  - nextjs-optimization-pack");
            println!("  - react-optimization-pack");
            println!("  - rust-axum-pack");
            println!("  - python-fastapi-pack");
        }
    }
    Ok(())
}

pub async fn scan(path: String) -> anyhow::Result<()> {
    let pkg_json = std::path::Path::new(&path).join("package.json");
    let cargo_toml = std::path::Path::new(&path).join("Cargo.toml");

    let (manifest_type, manifest_content) = if pkg_json.exists() {
        ("package_json", std::fs::read_to_string(&pkg_json)?)
    } else if cargo_toml.exists() {
        ("cargo_toml", std::fs::read_to_string(&cargo_toml)?)
    } else {
        println!("No package.json or Cargo.toml found in {path}");
        return Ok(());
    };

    let client = reqwest::Client::new();
    match client
        .post("http://localhost:8080/v1/remediation/scan")
        .json(&serde_json::json!({
            "manifest_type": manifest_type,
            "manifest_content": manifest_content
        }))
        .send()
        .await
    {
        Ok(r) => {
            let body: serde_json::Value = r.json().await?;
            println!("{body:#}");
        }
        Err(_) => println!("Server not running. Start with: aero serve"),
    }
    Ok(())
}
