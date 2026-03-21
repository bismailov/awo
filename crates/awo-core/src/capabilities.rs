use crate::runtime::{RuntimeKind, SessionLaunchMode};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, IntoStaticStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum CapabilitySupport {
    Native,
    ViaMcp,
    AdapterManaged,
    Planned,
    Unknown,
    Unsupported,
}

impl CapabilitySupport {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCapabilityDescriptor {
    pub runtime: String,
    pub display_name: String,
    pub default_launch_mode: String,
    pub inline_subagents: CapabilitySupport,
    pub multi_session_teams: CapabilitySupport,
    pub skill_preload: CapabilitySupport,
    pub persistent_subagent_memory: CapabilitySupport,
    pub reasoning_mcp_tools: CapabilitySupport,
    pub interrupt: CapabilitySupport,
    pub resume: CapabilitySupport,
    pub structured_output: CapabilitySupport,
    pub read_only_hint: CapabilitySupport,
    pub notes: Vec<String>,
}

pub fn runtime_capabilities(runtime: RuntimeKind) -> RuntimeCapabilityDescriptor {
    match runtime {
        RuntimeKind::Claude => RuntimeCapabilityDescriptor {
            runtime: runtime.as_str().to_string(),
            display_name: "Claude Code".to_string(),
            default_launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            inline_subagents: CapabilitySupport::Native,
            multi_session_teams: CapabilitySupport::Native,
            skill_preload: CapabilitySupport::Native,
            persistent_subagent_memory: CapabilitySupport::Native,
            reasoning_mcp_tools: CapabilitySupport::ViaMcp,
            interrupt: CapabilitySupport::Planned,
            resume: CapabilitySupport::Unknown,
            structured_output: CapabilitySupport::Unknown,
            read_only_hint: CapabilitySupport::Native,
            notes: vec![
                "Claude Code has official subagent and agent-team features.".to_string(),
                "Agent teams are experimental and should remain an adapter capability, not the core awo model.".to_string(),
            ],
        },
        RuntimeKind::Codex => RuntimeCapabilityDescriptor {
            runtime: runtime.as_str().to_string(),
            display_name: "Codex CLI".to_string(),
            default_launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            inline_subagents: CapabilitySupport::Unknown,
            multi_session_teams: CapabilitySupport::Unsupported,
            skill_preload: CapabilitySupport::AdapterManaged,
            persistent_subagent_memory: CapabilitySupport::Unknown,
            reasoning_mcp_tools: CapabilitySupport::ViaMcp,
            interrupt: CapabilitySupport::Planned,
            resume: CapabilitySupport::Unsupported,
            structured_output: CapabilitySupport::Unsupported,
            read_only_hint: CapabilitySupport::Native,
            notes: vec![
                "Codex is currently treated as a one-shot runtime in awo.".to_string(),
                "MCP-backed reasoning tools such as sequential thinking can be layered underneath without changing the orchestration model.".to_string(),
            ],
        },
        RuntimeKind::Gemini => RuntimeCapabilityDescriptor {
            runtime: runtime.as_str().to_string(),
            display_name: "Gemini CLI".to_string(),
            default_launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            inline_subagents: CapabilitySupport::Unknown,
            multi_session_teams: CapabilitySupport::Unsupported,
            skill_preload: CapabilitySupport::Native,
            persistent_subagent_memory: CapabilitySupport::Unknown,
            reasoning_mcp_tools: CapabilitySupport::ViaMcp,
            interrupt: CapabilitySupport::Planned,
            resume: CapabilitySupport::Unsupported,
            structured_output: CapabilitySupport::Unknown,
            read_only_hint: CapabilitySupport::Native,
            notes: vec![
                "Gemini already surfaces project-local skills, so awo should prefer repo-local context over heavy global projection.".to_string(),
            ],
        },
        RuntimeKind::Shell => RuntimeCapabilityDescriptor {
            runtime: runtime.as_str().to_string(),
            display_name: "Shell".to_string(),
            default_launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            inline_subagents: CapabilitySupport::Unsupported,
            multi_session_teams: CapabilitySupport::Unsupported,
            skill_preload: CapabilitySupport::Unsupported,
            persistent_subagent_memory: CapabilitySupport::Unsupported,
            reasoning_mcp_tools: CapabilitySupport::Unsupported,
            interrupt: CapabilitySupport::Planned,
            resume: CapabilitySupport::Unsupported,
            structured_output: CapabilitySupport::Unsupported,
            read_only_hint: CapabilitySupport::Unsupported,
            notes: vec![
                "Shell is a validation runtime for orchestration paths, not a full AI adapter.".to_string(),
            ],
        },
    }
}

pub fn all_runtime_capabilities() -> Vec<RuntimeCapabilityDescriptor> {
    [
        RuntimeKind::Codex,
        RuntimeKind::Claude,
        RuntimeKind::Gemini,
        RuntimeKind::Shell,
    ]
    .into_iter()
    .map(runtime_capabilities)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_capabilities_reflect_team_features() {
        let capabilities = runtime_capabilities(RuntimeKind::Claude);
        assert_eq!(capabilities.inline_subagents, CapabilitySupport::Native);
        assert_eq!(capabilities.multi_session_teams, CapabilitySupport::Native);
        assert_eq!(capabilities.skill_preload, CapabilitySupport::Native);
    }

    #[test]
    fn codex_capabilities_reflect_current_awo_model() {
        let capabilities = runtime_capabilities(RuntimeKind::Codex);
        assert_eq!(capabilities.default_launch_mode, "oneshot");
        assert_eq!(
            capabilities.structured_output,
            CapabilitySupport::Unsupported
        );
    }
}
