use aero_protocol::remediation::{ComplianceCheck, ComplianceLog};
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub struct ComplianceBuilder;

impl ComplianceBuilder {
    pub fn build(checks: Vec<ComplianceCheck>) -> ComplianceLog {
        let log_id = format!("aero-compliance-{}", chrono::Utc::now().timestamp());
        let timestamp = chrono::Utc::now();

        let checks_json = serde_json::to_string(&checks).unwrap_or_default();

        let mut hmac = Hmac::<Sha256>::new_from_slice(b"aero_compliance_signing_key").unwrap();
        hmac.update(checks_json.as_bytes());
        let signature = hex::encode(hmac.finalize().into_bytes());

        ComplianceLog {
            log_id,
            timestamp,
            checks,
            signature,
        }
    }
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}
