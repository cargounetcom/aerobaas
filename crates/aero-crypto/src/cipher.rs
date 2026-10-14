use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};

pub struct AesCipher {
    cipher: Aes256Gcm,
}

impl AesCipher {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new(key.into()),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("AES-GCM encrypt failed: {e}"))?;
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if data.len() < 12 {
            anyhow::bail!("Ciphertext too short for nonce");
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("AES-GCM decrypt failed: {e}"))?;
        Ok(plaintext)
    }

    pub fn encrypt_str(&self, plaintext: &str) -> anyhow::Result<String> {
        let bytes = self.encrypt(plaintext.as_bytes())?;
        Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes))
    }

    pub fn decrypt_str(&self, encoded: &str) -> anyhow::Result<String> {
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)?;
        let plaintext = self.decrypt(&bytes)?;
        Ok(String::from_utf8(plaintext)?)
    }
}
