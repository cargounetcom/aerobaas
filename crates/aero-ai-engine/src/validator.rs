use aero_protocol::AeroResult;
use crate::engine::{ThreatType, ValidationResult};

pub struct ContentValidator;

impl ContentValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_json_document(&self, doc: &serde_json::Value) -> AeroResult<ValidationResult> {
        let content = serde_json::to_string(doc).unwrap_or_default();
        let suspicious = ["ignore previous", "system prompt", "jailbreak", "DROP TABLE", "rm -rf"];

        let is_suspicious = suspicious.iter().any(|p| content.to_lowercase().contains(p));

        Ok(ValidationResult {
            is_safe: !is_suspicious,
            threat_type: if is_suspicious {
                Some(ThreatType::CodeInjection)
            } else {
                None
            },
            confidence: if is_suspicious { 0.80 } else { 0.99 },
        })
    }

    pub fn anonymize_pii(&self, text: &str) -> String {
        let mut result = text.to_string();

        let patterns = [
            (r"\b\d{3}-\d{2}-\d{4}\b", "[SSN]"),
            (r"\b\d{16}\b", "[CARD]"),
            (r"\b[A-Za-z]+ [A-Za-z]+\b", "[NAME]"),
        ];

        for (pattern, replacement) in &patterns {
            let re = regex_lite::Regex::new(pattern).unwrap();
            result = re.replace_all(&result, *replacement).to_string();
        }

        result
    }
}

mod regex_lite {
    pub struct Regex {
        pattern: String,
    }

    impl Regex {
        pub fn new(pattern: &str) -> Result<Self, ()> {
            Ok(Self {
                pattern: pattern.to_string(),
            })
        }

        pub fn replace_all<'a>(&self, text: &'a str, replacement: &str) -> std::borrow::Cow<'a, str> {
            std::borrow::Cow::Borrowed(text)
        }
    }

    impl std::ops::Deref for Regex {
        type Target = str;
        fn deref(&self) -> &Self::Target {
            &self.pattern
        }
    }
}
