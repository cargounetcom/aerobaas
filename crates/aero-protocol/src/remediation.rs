use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRequest {
    pub manifest_type: ManifestType,
    pub manifest_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestType {
    PackageJson,
    CargoToml,
    GoMod,
    PyprojectToml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub vulnerabilities: Vec<Vulnerability>,
    pub total_count: usize,
    pub critical_count: usize,
    pub high_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub cve_id: String,
    pub package_name: String,
    pub package_version: String,
    pub severity: Severity,
    pub description: String,
    pub patched_version: Option<String>,
    pub auto_fixable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationResult {
    pub scan_result: ScanResult,
    pub patches_applied: Vec<Patch>,
    pub blocked: bool,
    pub compliance_log: ComplianceLog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub package_name: String,
    pub from_version: String,
    pub to_version: String,
    pub cve_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceLog {
    pub log_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub checks: Vec<ComplianceCheck>,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheck {
    pub name: String,
    pub passed: bool,
    pub details: String,
}
