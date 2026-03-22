use crate::capabilities::{CostTier, LimitProfile, runtime_capabilities};
use crate::runtime::RuntimeKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingTarget {
    pub runtime: RuntimeKind,
    pub model: Option<String>,
}

impl RoutingTarget {
    pub fn new(runtime: RuntimeKind, model: Option<String>) -> Self {
        Self { runtime, model }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingPreferences {
    pub allow_fallback: bool,
    pub prefer_local: bool,
    pub avoid_metered: bool,
    pub max_cost_tier: Option<CostTier>,
}

impl Default for RoutingPreferences {
    fn default() -> Self {
        Self {
            allow_fallback: true,
            prefer_local: false,
            avoid_metered: false,
            max_cost_tier: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingSource {
    Primary,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub selected_runtime: RuntimeKind,
    pub selected_model: Option<String>,
    pub source: RoutingSource,
    pub reason: String,
}

pub fn route_runtime(
    primary: RoutingTarget,
    fallback: Option<RoutingTarget>,
    preferences: &RoutingPreferences,
) -> RoutingDecision {
    let primary_check = evaluate_target(&primary, preferences);
    if primary_check.accepted {
        return RoutingDecision {
            selected_runtime: primary.runtime,
            selected_model: primary.model,
            source: RoutingSource::Primary,
            reason: "primary target meets all routing preferences".to_string(),
        };
    }

    if preferences.allow_fallback
        && let Some(fallback) = fallback
    {
        let fallback_check = evaluate_target(&fallback, preferences);
        if fallback_check.accepted {
            return RoutingDecision {
                selected_runtime: fallback.runtime,
                selected_model: fallback.model,
                source: RoutingSource::Fallback,
                reason: format!(
                    "primary rejected ({}) and fallback accepted",
                    primary_check.reason
                ),
            };
        }
        return RoutingDecision {
            selected_runtime: primary.runtime,
            selected_model: primary.model,
            source: RoutingSource::Primary,
            reason: format!(
                "both primary ({}) and fallback ({}) were rejected; defaulting to primary",
                primary_check.reason, fallback_check.reason
            ),
        };
    }

    RoutingDecision {
        selected_runtime: primary.runtime,
        selected_model: primary.model,
        source: RoutingSource::Primary,
        reason: if preferences.allow_fallback {
            format!(
                "primary rejected ({}) and no fallback target was available; defaulting to primary",
                primary_check.reason
            )
        } else {
            format!(
                "primary rejected ({}) but fallback was not allowed; defaulting to primary",
                primary_check.reason
            )
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetCheck {
    accepted: bool,
    reason: String,
}

fn evaluate_target(target: &RoutingTarget, preferences: &RoutingPreferences) -> TargetCheck {
    let capabilities = runtime_capabilities(target.runtime);

    if preferences.prefer_local && capabilities.cost_tier != CostTier::Local {
        return TargetCheck {
            accepted: false,
            reason: "prefer_local requires a local runtime".to_string(),
        };
    }

    if preferences.avoid_metered && capabilities.limit_profile == LimitProfile::ApiMetered {
        return TargetCheck {
            accepted: false,
            reason: "avoid_metered rejects metered runtimes".to_string(),
        };
    }

    if let Some(max_cost_tier) = preferences.max_cost_tier
        && !cost_within_limit(capabilities.cost_tier, max_cost_tier)
    {
        return TargetCheck {
            accepted: false,
            reason: format!(
                "cost tier {} exceeds max {}",
                capabilities.cost_tier.as_str(),
                max_cost_tier.as_str()
            ),
        };
    }

    TargetCheck {
        accepted: true,
        reason: "accepted".to_string(),
    }
}

fn cost_within_limit(cost_tier: CostTier, max_cost_tier: CostTier) -> bool {
    match (cost_rank(cost_tier), cost_rank(max_cost_tier)) {
        (Some(cost_rank), Some(max_rank)) => cost_rank <= max_rank,
        _ => false,
    }
}

fn cost_rank(cost_tier: CostTier) -> Option<u8> {
    match cost_tier {
        CostTier::Local => Some(0),
        CostTier::Cheap => Some(1),
        CostTier::Standard => Some(2),
        CostTier::Premium => Some(3),
        CostTier::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_chosen_by_default() {
        let preferences = RoutingPreferences::default();
        let decision = route_runtime(
            RoutingTarget::new(RuntimeKind::Claude, Some("sonnet".to_string())),
            Some(RoutingTarget::new(
                RuntimeKind::Gemini,
                Some("flash".to_string()),
            )),
            &preferences,
        );
        assert_eq!(decision.selected_runtime, RuntimeKind::Claude);
        assert_eq!(decision.selected_model.as_deref(), Some("sonnet"));
        assert_eq!(decision.source, RoutingSource::Primary);
    }

    #[test]
    fn fallback_chosen_when_primary_violates_cost_ceiling() {
        let preferences = RoutingPreferences {
            max_cost_tier: Some(CostTier::Standard),
            ..Default::default()
        };
        let decision = route_runtime(
            RoutingTarget::new(RuntimeKind::Claude, Some("opus".to_string())),
            Some(RoutingTarget::new(
                RuntimeKind::Gemini,
                Some("flash".to_string()),
            )),
            &preferences,
        );
        assert_eq!(decision.selected_runtime, RuntimeKind::Gemini);
        assert_eq!(decision.selected_model.as_deref(), Some("flash"));
        assert_eq!(decision.source, RoutingSource::Fallback);
    }

    #[test]
    fn fallback_rejected_when_allow_fallback_is_false() {
        let preferences = RoutingPreferences {
            max_cost_tier: Some(CostTier::Standard),
            allow_fallback: false,
            ..Default::default()
        };
        let decision = route_runtime(
            RoutingTarget::new(RuntimeKind::Claude, Some("opus".to_string())),
            Some(RoutingTarget::new(
                RuntimeKind::Gemini,
                Some("flash".to_string()),
            )),
            &preferences,
        );
        assert_eq!(decision.selected_runtime, RuntimeKind::Claude);
        assert_eq!(decision.selected_model.as_deref(), Some("opus"));
        assert_eq!(decision.source, RoutingSource::Primary);
        assert!(decision.reason.contains("fallback was not allowed"));
    }

    #[test]
    fn local_preference_behavior() {
        let preferences = RoutingPreferences {
            prefer_local: true,
            ..Default::default()
        };
        let decision = route_runtime(
            RoutingTarget::new(RuntimeKind::Claude, Some("sonnet".to_string())),
            Some(RoutingTarget::new(RuntimeKind::Shell, None)),
            &preferences,
        );
        assert_eq!(decision.selected_runtime, RuntimeKind::Shell);
        assert_eq!(decision.source, RoutingSource::Fallback);
    }

    #[test]
    fn metered_avoidance_behavior() {
        let preferences = RoutingPreferences {
            avoid_metered: true,
            ..Default::default()
        };
        let decision = route_runtime(
            RoutingTarget::new(RuntimeKind::Codex, Some("gpt-5.4".to_string())),
            Some(RoutingTarget::new(RuntimeKind::Shell, None)),
            &preferences,
        );
        assert_eq!(decision.selected_runtime, RuntimeKind::Shell);
        assert_eq!(decision.source, RoutingSource::Fallback);
    }

    #[test]
    fn unknown_cost_tier_is_rejected_by_cost_ceiling() {
        assert!(!cost_within_limit(CostTier::Unknown, CostTier::Standard));
    }
}
