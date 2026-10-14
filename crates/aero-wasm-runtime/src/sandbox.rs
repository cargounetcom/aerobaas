pub struct SandboxConfig {
    pub memory_limit_mb: u32,
    pub cpu_limit_percent: u32,
    pub timeout_ms: u32,
}

impl SandboxConfig {
    pub fn new(memory_limit_mb: u32, cpu_limit_percent: u32, timeout_ms: u32) -> Self {
        Self {
            memory_limit_mb,
            cpu_limit_percent,
            timeout_ms,
        }
    }

    pub fn memory_limit_bytes(&self) -> u64 {
        (self.memory_limit_mb as u64) * 1024 * 1024
    }

    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.timeout_ms as u64)
    }
}
