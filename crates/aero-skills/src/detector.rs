use std::path::Path;

pub struct FrameworkDetector;

impl FrameworkDetector {
    pub fn detect(workspace_path: &str) -> String {
        let path = Path::new(workspace_path);

        if path.join("package.json").exists() {
            let content = std::fs::read_to_string(path.join("package.json")).unwrap_or_default();
            if content.contains("\"next\"") {
                return "nextjs".into();
            }
            if content.contains("\"react\"") {
                return "react".into();
            }
            if content.contains("\"vue\"") {
                return "vue".into();
            }
            if content.contains("\"svelte\"") {
                return "svelte".into();
            }
            return "node".into();
        }

        if path.join("Cargo.toml").exists() {
            let content = std::fs::read_to_string(path.join("Cargo.toml")).unwrap_or_default();
            if content.contains("axum") {
                return "rust-axum".into();
            }
            if content.contains("actix") {
                return "rust-actix".into();
            }
            return "rust".into();
        }

        if path.join("go.mod").exists() {
            return "go".into();
        }

        if path.join("pyproject.toml").exists() || path.join("requirements.txt").exists() {
            let content = std::fs::read_to_string(path.join("pyproject.toml"))
                .unwrap_or_default();
            if content.contains("fastapi") {
                return "python-fastapi".into();
            }
            if content.contains("django") {
                return "python-django".into();
            }
            return "python".into();
        }

        "unknown".into()
    }
}
