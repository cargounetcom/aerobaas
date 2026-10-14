use aero_protocol::remediation::{ManifestType, ScanRequest, ScanResult, Severity, Vulnerability};

pub struct Scanner;

impl Scanner {
    pub fn new() -> Self {
        Self
    }

    pub async fn scan(&self, req: &ScanRequest) -> ScanResult {
        let packages = match req.manifest_type {
            ManifestType::PackageJson => parse_package_json(&req.manifest_content),
            ManifestType::CargoToml => parse_cargo_toml(&req.manifest_content),
            ManifestType::GoMod => parse_go_mod(&req.manifest_content),
            ManifestType::PyprojectToml => parse_pyproject_toml(&req.manifest_content),
        };

        let mut vulnerabilities = Vec::new();

        for (name, version) in &packages {
            if let Some(vuln) = check_known_vulns(name, version) {
                vulnerabilities.push(vuln);
            }
        }

        let critical_count = vulnerabilities
            .iter()
            .filter(|v| matches!(v.severity, Severity::Critical))
            .count();
        let high_count = vulnerabilities
            .iter()
            .filter(|v| matches!(v.severity, Severity::High))
            .count();

        ScanResult {
            total_count: vulnerabilities.len(),
            critical_count,
            high_count,
            vulnerabilities,
        }
    }
}

fn parse_package_json(content: &str) -> Vec<(String, String)> {
    let mut packages = Vec::new();
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
            for (name, ver) in deps {
                packages.push((name.clone(), ver.as_str().unwrap_or("").trim_start_matches('^').to_string()));
            }
        }
        if let Some(deps) = json.get("devDependencies").and_then(|d| d.as_object()) {
            for (name, ver) in deps {
                packages.push((name.clone(), ver.as_str().unwrap_or("").trim_start_matches('^').to_string()));
            }
        }
    }
    packages
}

fn parse_cargo_toml(content: &str) -> Vec<(String, String)> {
    let mut packages = Vec::new();
    let mut in_deps = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed.contains("dependencies");
            continue;
        }
        if in_deps {
            if let Some(eq_pos) = trimmed.find('=') {
                let name = trimmed[..eq_pos].trim().trim_matches('"').to_string();
                let version = trimmed[eq_pos + 1..]
                    .trim()
                    .trim_start_matches('"')
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches('"')
                    .to_string();
                if !name.is_empty() {
                    packages.push((name, version));
                }
            }
        }
    }
    packages
}

fn parse_go_mod(content: &str) -> Vec<(String, String)> {
    let mut packages = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("require ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                packages.push((parts[1].to_string(), parts[2].to_string()));
            }
        }
    }
    packages
}

fn parse_pyproject_toml(content: &str) -> Vec<(String, String)> {
    let mut packages = Vec::new();
    let mut in_deps = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("dependencies") && trimmed.contains('=') {
            in_deps = true;
            continue;
        }
        if in_deps {
            if trimmed.starts_with(']') || (trimmed.starts_with('[') && !trimmed.contains("dependencies")) {
                in_deps = false;
                continue;
            }
            let cleaned = trimmed.trim_matches(|c| c == '"' || c == ',' || c == ' ');
            if !cleaned.is_empty() && !cleaned.starts_with('[') && !cleaned.starts_with(']') {
                let parts: Vec<&str> = cleaned.splitn(2, |c: char| c == '=' || c == '>' || c == '<' || c == '~' || c == '!').collect();
                let name = parts[0].trim().to_string();
                let version = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
                packages.push((name, version));
            }
        }
    }
    packages
}

fn check_known_vulns(name: &str, version: &str) -> Option<Vulnerability> {
    let known_vulns: &[(&str, &str, &str, Severity, &str, Option<&str>)] = &[
        ("lodash", "4.17.20", "CVE-2021-23337", Severity::High, "Command injection via template", Some("4.17.21")),
        ("axios", "0.21.0", "CVE-2021-3749", Severity::High, "SSRF via absolute URL", Some("0.21.1")),
        ("log4j", "2.14.0", "CVE-2021-44228", Severity::Critical, "Remote code execution via JNDI", Some("2.17.1")),
        ("minimist", "1.2.5", "CVE-2021-44906", Severity::Critical, "Prototype pollution", Some("1.2.6")),
        ("jsonwebtoken", "8.5.1", "CVE-2022-23529", Severity::Critical, "Algorithm confusion attack", Some("9.0.0")),
    ];

    for (vuln_name, vuln_ver, cve, severity, desc, patch) in known_vulns {
        if name == *vuln_name && version.starts_with(vuln_ver.split('.').take(2).collect::<Vec<_>>().join(".").as_str()) {
            return Some(Vulnerability {
                cve_id: cve.to_string(),
                package_name: name.to_string(),
                package_version: version.to_string(),
                severity: severity.clone(),
                description: desc.to_string(),
                patched_version: patch.map(|s| s.to_string()),
                auto_fixable: patch.is_some(),
            });
        }
    }

    None
}
