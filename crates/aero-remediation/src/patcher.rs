use aero_protocol::remediation::{Patch, ScanResult};

pub struct Patcher;

impl Patcher {
    pub fn new() -> Self {
        Self
    }

    pub async fn auto_patch(&self, scan_result: &ScanResult) -> Vec<Patch> {
        let mut patches = Vec::new();

        for vuln in &scan_result.vulnerabilities {
            if vuln.auto_fixable {
                if let Some(ref patched_version) = vuln.patched_version {
                    tracing::info!(
                        package = %vuln.package_name,
                        from = %vuln.package_version,
                        to = %patched_version,
                        cve = %vuln.cve_id,
                        "Auto-patching vulnerable dependency"
                    );
                    patches.push(Patch {
                        package_name: vuln.package_name.clone(),
                        from_version: vuln.package_version.clone(),
                        to_version: patched_version.clone(),
                        cve_ids: vec![vuln.cve_id.clone()],
                    });
                }
            }
        }

        patches
    }
}
