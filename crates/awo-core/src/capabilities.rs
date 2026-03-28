use crate::runtime::{
    RuntimeKind, SessionCapacityStatus, SessionEndReason, SessionLaunchMode, SessionStatus,
};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString, IntoStaticStr};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CostTier {
    Local,
    Cheap,
    Standard,
    Premium,
    Unknown,
}

impl CostTier {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum LimitProfile {
    LocalUnlimited,
    ApiMetered,
    SeatWithSoftLimits,
    Unknown,
}

impl LimitProfile {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCapabilityDescriptor {
    pub runtime: String,
    pub display_name: String,
    pub default_launch_mode: String,
    pub cost_tier: CostTier,
    pub limit_profile: LimitProfile,
    pub usage_reporting: CapabilitySupport,
    pub capacity_reporting: CapabilitySupport,
    pub budget_guardrails: CapabilitySupport,
    pub session_lifetime: CapabilitySupport,
    pub operator_note: String,
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
            cost_tier: CostTier::Premium,
            limit_profile: LimitProfile::ApiMetered,
            usage_reporting: CapabilitySupport::Planned,
            capacity_reporting: CapabilitySupport::Planned,
            budget_guardrails: CapabilitySupport::Native,
            session_lifetime: CapabilitySupport::AdapterManaged,
            operator_note: "High intelligence, higher spend. Use for complex planning and difficult code review."
                .to_string(),
            inline_subagents: CapabilitySupport::Native,
            multi_session_teams: CapabilitySupport::Native,
            skill_preload: CapabilitySupport::Native,
            persistent_subagent_memory: CapabilitySupport::Native,
            reasoning_mcp_tools: CapabilitySupport::ViaMcp,
            interrupt: CapabilitySupport::Planned,
            resume: CapabilitySupport::Unknown,
            structured_output: CapabilitySupport::Native,
            read_only_hint: CapabilitySupport::Native,
            notes: vec![
                "Claude Code has official subagent and agent-team features.".to_string(),
                "Agent teams are experimental and should remain an adapter capability, not the core awo model.".to_string(),
                "Anthropic exposes usage and cost data at the provider layer, but the current awo CLI adapter does not ingest it yet.".to_string(),
                "Claude CLI exposes `--max-budget-usd`, JSON output, and JSON-schema validation in print mode.".to_string(),
            ],
        },
        RuntimeKind::Codex => RuntimeCapabilityDescriptor {
            runtime: runtime.as_str().to_string(),
            display_name: "Codex CLI".to_string(),
            default_launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            cost_tier: CostTier::Standard,
            limit_profile: LimitProfile::ApiMetered,
            usage_reporting: CapabilitySupport::Planned,
            capacity_reporting: CapabilitySupport::Planned,
            budget_guardrails: CapabilitySupport::Unknown,
            session_lifetime: CapabilitySupport::AdapterManaged,
            operator_note: "Good default balance for one-shot implementation and review loops."
                .to_string(),
            inline_subagents: CapabilitySupport::Unknown,
            multi_session_teams: CapabilitySupport::Unsupported,
            skill_preload: CapabilitySupport::AdapterManaged,
            persistent_subagent_memory: CapabilitySupport::Unknown,
            reasoning_mcp_tools: CapabilitySupport::ViaMcp,
            interrupt: CapabilitySupport::Planned,
            resume: CapabilitySupport::Unsupported,
            structured_output: CapabilitySupport::Native,
            read_only_hint: CapabilitySupport::Native,
            notes: vec![
                "Codex is currently treated as a one-shot runtime in awo.".to_string(),
                "MCP-backed reasoning tools such as sequential thinking can be layered underneath without changing the orchestration model.".to_string(),
                "OpenAI exposes usage and cost data at the provider layer, but the current awo CLI adapter does not ingest it yet.".to_string(),
                "Codex exec exposes JSON event output and JSON-schema-constrained final responses.".to_string(),
            ],
        },
        RuntimeKind::Gemini => RuntimeCapabilityDescriptor {
            runtime: runtime.as_str().to_string(),
            display_name: "Gemini CLI".to_string(),
            default_launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            cost_tier: CostTier::Cheap,
            limit_profile: LimitProfile::ApiMetered,
            usage_reporting: CapabilitySupport::Planned,
            capacity_reporting: CapabilitySupport::Planned,
            budget_guardrails: CapabilitySupport::Unknown,
            session_lifetime: CapabilitySupport::AdapterManaged,
            operator_note: "Useful for large-context reads, audits, and lower-cost fallback work."
                .to_string(),
            inline_subagents: CapabilitySupport::Unknown,
            multi_session_teams: CapabilitySupport::Unsupported,
            skill_preload: CapabilitySupport::Native,
            persistent_subagent_memory: CapabilitySupport::Unknown,
            reasoning_mcp_tools: CapabilitySupport::ViaMcp,
            interrupt: CapabilitySupport::Planned,
            resume: CapabilitySupport::Unsupported,
            structured_output: CapabilitySupport::Native,
            read_only_hint: CapabilitySupport::Native,
            notes: vec![
                "Gemini already surfaces project-local skills, so awo should prefer repo-local context over heavy global projection.".to_string(),
                "Google exposes usage and quota data at the provider layer, but the current awo CLI adapter does not ingest it yet.".to_string(),
                "Gemini CLI exposes JSON and stream-json output modes in headless operation.".to_string(),
            ],
        },
        RuntimeKind::Shell => RuntimeCapabilityDescriptor {
            runtime: runtime.as_str().to_string(),
            display_name: "Shell".to_string(),
            default_launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            cost_tier: CostTier::Local,
            limit_profile: LimitProfile::LocalUnlimited,
            usage_reporting: CapabilitySupport::Unsupported,
            capacity_reporting: CapabilitySupport::Unsupported,
            budget_guardrails: CapabilitySupport::Unsupported,
            session_lifetime: CapabilitySupport::AdapterManaged,
            operator_note: "Local validation and orchestration helper with no model quota pressure."
                .to_string(),
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

pub fn usage_note_for_runtime(runtime: RuntimeKind) -> String {
    match runtime {
        RuntimeKind::Shell => {
            "Shell has no token budget and does not expose model-usage telemetry.".to_string()
        }
        RuntimeKind::Claude => "Structured usage stats are not available through the current Claude CLI adapter; inspect Anthropic usage APIs or dashboards for exact spend.".to_string(),
        RuntimeKind::Codex => "Structured usage stats are not available through the current Codex CLI adapter; inspect OpenAI usage APIs or dashboards for exact spend.".to_string(),
        RuntimeKind::Gemini => "Structured usage stats are not available through the current Gemini CLI adapter; inspect Google usage or quota dashboards for exact spend.".to_string(),
    }
}

pub fn session_recovery_guidance(
    runtime: RuntimeKind,
    status: SessionStatus,
    end_reason: Option<SessionEndReason>,
    capacity_status: SessionCapacityStatus,
) -> Option<String> {
    match status {
        SessionStatus::Prepared => Some(
            "Session is prepared but not yet running. Start it or replace it before continuing."
                .to_string(),
        ),
        SessionStatus::Running => None,
        SessionStatus::Completed => Some(
            "Session completed. Review diff/log output, then accept the task card or clean up its slot."
                .to_string(),
        ),
        SessionStatus::Cancelled => Some(
            "Session was cancelled by the operator. Restart it, reassign the task card, or close it out intentionally."
                .to_string(),
        ),
        SessionStatus::Failed => match end_reason {
            Some(SessionEndReason::Timeout) => Some(
                "Session timed out. Split the task card, narrow scope, or restart with a fresh handoff."
                    .to_string(),
            ),
            Some(SessionEndReason::TokenExhausted) => Some(
                "Session likely exhausted context or token budget. Hand off to another agent, reduce scope, or choose a different model."
                    .to_string(),
            ),
            Some(SessionEndReason::ProviderLimited) => Some(
                "Session hit a provider quota or rate limit. Retry later, reduce concurrency, switch models, or inspect billing and quota state."
                    .to_string(),
            ),
            Some(SessionEndReason::OperatorCancelled) => Some(
                "Session was cancelled explicitly. Restart it or reassign the task card if work should continue."
                    .to_string(),
            ),
            Some(SessionEndReason::RuntimeFailure) | Some(SessionEndReason::Completed) | None => {
                match runtime {
                    RuntimeKind::Shell => Some(
                        "Shell session failed. Inspect exit code and logs, then retry only after fixing the command or environment."
                            .to_string(),
                    ),
                    _ if capacity_status == SessionCapacityStatus::Unknown => Some(
                        "Session failed without structured usage telemetry. Inspect logs; this may be a runtime failure, timeout, or provider-side budget exhaustion."
                            .to_string(),
                    ),
                    _ => Some(
                        "Session failed. Inspect logs and either retry with narrower scope or hand the task card to another agent."
                            .to_string(),
                    ),
                }
            }
        },
    }
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
        assert_eq!(capabilities.cost_tier, CostTier::Premium);
        assert_eq!(capabilities.limit_profile, LimitProfile::ApiMetered);
        assert_eq!(capabilities.usage_reporting, CapabilitySupport::Planned);
        assert_eq!(capabilities.capacity_reporting, CapabilitySupport::Planned);
        assert_eq!(capabilities.budget_guardrails, CapabilitySupport::Native);
        assert_eq!(capabilities.structured_output, CapabilitySupport::Native);
        assert_eq!(
            capabilities.session_lifetime,
            CapabilitySupport::AdapterManaged
        );
    }

    #[test]
    fn codex_capabilities_reflect_current_awo_model() {
        let capabilities = runtime_capabilities(RuntimeKind::Codex);
        assert_eq!(capabilities.default_launch_mode, "oneshot");
        assert_eq!(capabilities.structured_output, CapabilitySupport::Native);
        assert_eq!(capabilities.cost_tier, CostTier::Standard);
        assert_eq!(capabilities.usage_reporting, CapabilitySupport::Planned);
        assert_eq!(capabilities.capacity_reporting, CapabilitySupport::Planned);
        assert_eq!(capabilities.budget_guardrails, CapabilitySupport::Unknown);
    }

    #[test]
    fn gemini_capabilities_reflect_headless_output_support() {
        let capabilities = runtime_capabilities(RuntimeKind::Gemini);
        assert_eq!(capabilities.structured_output, CapabilitySupport::Native);
        assert_eq!(capabilities.usage_reporting, CapabilitySupport::Planned);
        assert_eq!(capabilities.capacity_reporting, CapabilitySupport::Planned);
    }

    #[test]
    fn shell_capabilities_reflect_local_runtime_profile() {
        let capabilities = runtime_capabilities(RuntimeKind::Shell);
        assert_eq!(capabilities.cost_tier, CostTier::Local);
        assert_eq!(capabilities.limit_profile, LimitProfile::LocalUnlimited);
        assert_eq!(
            capabilities.budget_guardrails,
            CapabilitySupport::Unsupported
        );
    }

    #[test]
    fn session_recovery_guidance_distinguishes_timeout_exhaustion_and_provider_limits() {
        let timeout = session_recovery_guidance(
            RuntimeKind::Codex,
            SessionStatus::Failed,
            Some(SessionEndReason::Timeout),
            SessionCapacityStatus::TimedOut,
        )
        .expect("timeout guidance");
        assert!(timeout.contains("timed out"));

        let exhausted = session_recovery_guidance(
            RuntimeKind::Claude,
            SessionStatus::Failed,
            Some(SessionEndReason::TokenExhausted),
            SessionCapacityStatus::Exhausted,
        )
        .expect("exhaustion guidance");
        assert!(exhausted.contains("token budget"));

        let limited = session_recovery_guidance(
            RuntimeKind::Codex,
            SessionStatus::Failed,
            Some(SessionEndReason::ProviderLimited),
            SessionCapacityStatus::ProviderLimited,
        )
        .expect("provider limit guidance");
        assert!(limited.contains("quota or rate limit"));
    }

    #[test]
    fn usage_notes_point_to_provider_truth_sources() {
        assert!(usage_note_for_runtime(RuntimeKind::Claude).contains("Anthropic"));
        assert!(usage_note_for_runtime(RuntimeKind::Codex).contains("OpenAI"));
        assert!(usage_note_for_runtime(RuntimeKind::Gemini).contains("Google"));
        assert!(usage_note_for_runtime(RuntimeKind::Shell).contains("no token budget"));
    }
}
