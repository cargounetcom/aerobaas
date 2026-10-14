use std::collections::HashSet;

pub struct EgressFilter {
    allowed_hosts: HashSet<String>,
    enabled: bool,
}

impl EgressFilter {
    pub fn new(allowed_hosts: Vec<String>, enabled: bool) -> Self {
        Self {
            allowed_hosts: allowed_hosts.into_iter().collect(),
            enabled,
        }
    }

    pub fn check_egress(&self, host: &str) -> EgressDecision {
        if !self.enabled {
            return EgressDecision::Allow;
        }

        if self.allowed_hosts.is_empty() {
            return EgressDecision::Deny;
        }

        if self.allowed_hosts.contains(host) {
            EgressDecision::Allow
        } else {
            tracing::warn!(host, "Egress denied by Hazmat filter");
            EgressDecision::Deny
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressDecision {
    Allow,
    Deny,
}
