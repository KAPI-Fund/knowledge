use serde::{Deserialize, Serialize};

use crate::tenancy::access::AccessRole;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    ReadProject,
    ReadSource,
    SearchWiki,
    SearchWeb,
    WriteWiki,
    Network,
    Process,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPolicy {
    allowed: Vec<AgentCapability>,
}

impl PermissionPolicy {
    /// Map the caller's tenancy role onto agent capabilities: every role with
    /// project access may read and search; write and sandbox process tools
    /// require Editor or better. shell.exec also requires explicit per-command
    /// user approval before execution.
    pub fn for_role(role: AccessRole) -> Self {
        let mut allowed = vec![
            AgentCapability::ReadProject,
            AgentCapability::ReadSource,
            AgentCapability::SearchWiki,
            AgentCapability::SearchWeb,
            AgentCapability::Network,
        ];
        if role.satisfies(AccessRole::Editor) {
            allowed.push(AgentCapability::WriteWiki);
            allowed.push(AgentCapability::Process);
        }
        Self { allowed }
    }

    pub fn allows(&self, capability: AgentCapability) -> bool {
        self.allowed.contains(&capability)
    }

    pub fn require(&self, capability: AgentCapability) -> Result<(), String> {
        if self.allows(capability) {
            Ok(())
        } else {
            Err(format!("Agent capability '{capability:?}' is not allowed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_gets_read_search_but_not_write() {
        let policy = PermissionPolicy::for_role(AccessRole::Viewer);
        assert!(policy.allows(AgentCapability::SearchWiki));
        assert!(policy.allows(AgentCapability::ReadSource));
        assert!(policy.allows(AgentCapability::Network));
        assert!(!policy.allows(AgentCapability::WriteWiki));
        assert!(!policy.allows(AgentCapability::Process));
    }

    #[test]
    fn editor_and_owner_get_wiki_writes_and_process() {
        for role in [AccessRole::Editor, AccessRole::Owner] {
            let policy = PermissionPolicy::for_role(role);
            assert!(policy.allows(AgentCapability::WriteWiki));
            assert!(policy.allows(AgentCapability::Process));
        }
    }
}
