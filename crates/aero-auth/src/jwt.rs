use aero_protocol::auth::JwtClaims;
use base64::Engine;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct JwtService {
    secret: Vec<u8>,
}

impl JwtService {
    pub fn new(secret: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        let hash = hasher.finalize();
        Self {
            secret: hash.to_vec(),
        }
    }

    pub fn issue(&self, claims: &JwtClaims) -> anyhow::Result<String> {
        let header = serde_json::json!({
            "alg": "HS256",
            "typ": "JWT"
        });
        let header_b64 = self.b64url_encode(&serde_json::to_vec(&header)?);
        let payload_b64 = self.b64url_encode(&serde_json::to_vec(&claims)?);
        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = self.hmac_sha256(&signing_input);
        let sig_b64 = self.b64url_encode(&signature);
        Ok(format!("{signing_input}.{sig_b64}"))
    }

    pub fn verify(&self, token: &str) -> anyhow::Result<JwtClaims> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid JWT format");
        }

        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let provided_sig = self.b64url_decode(parts[2])?;
        {
            use hmac::Mac;
            let mut mac = hmac::Hmac::<Sha256>::new_from_slice(&self.secret)
                .map_err(|_| anyhow::anyhow!("invalid key"))?;
            mac.update(signing_input.as_bytes());
            // constant-time comparison
            mac.verify_slice(&provided_sig)
                .map_err(|_| anyhow::anyhow!("Invalid JWT signature"))?;
        }

        let payload_bytes = self.b64url_decode(parts[1])?;
        let claims: JwtClaims = serde_json::from_slice(&payload_bytes)?;

        let now = chrono::Utc::now().timestamp() as usize;
        if claims.exp < now {
            anyhow::bail!("JWT expired");
        }

        Ok(claims)
    }

    pub fn issue_pair(
        &self,
        user_id: Uuid,
        email: &str,
        workspace_id: Option<Uuid>,
        role: Option<&str>,
        scopes: &[String],
    ) -> anyhow::Result<(String, String)> {
        let now = chrono::Utc::now().timestamp() as usize;
        let access_claims = JwtClaims {
            sub: user_id,
            email: email.to_string(),
            workspace_id,
            role: role.map(|r| r.to_string()),
            scopes: scopes.to_vec(),
            exp: now + 3600,
            iat: now,
            typ: "access".into(),
        };
        let refresh_claims = JwtClaims {
            sub: user_id,
            email: email.to_string(),
            workspace_id,
            role: role.map(|r| r.to_string()),
            scopes: scopes.to_vec(),
            exp: now + 86400 * 30,
            iat: now,
            typ: "refresh".into(),
        };
        let access = self.issue(&access_claims)?;
        let refresh = self.issue(&refresh_claims)?;
        Ok((access, refresh))
    }

    fn hmac_sha256(&self, data: &str) -> Vec<u8> {
        use hmac::Mac;
        let mut mac = hmac::Hmac::<Sha256>::new_from_slice(&self.secret).unwrap();
        mac.update(data.as_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    fn b64url_encode(&self, data: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
    }

    fn b64url_decode(&self, s: &str) -> anyhow::Result<Vec<u8>> {
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)?)
    }
}
