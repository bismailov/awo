pub mod app;
pub mod capabilities;
pub mod commands;
pub mod config;
pub mod context;
pub mod diagnostics;
pub mod error;
pub mod events;
pub mod fingerprint;
pub mod git;
pub mod platform;
pub mod repo;
pub mod routing;
pub mod runtime;
pub mod skills;
pub mod slot;
pub mod snapshot;
pub mod store;
pub mod team;

pub use app::{AppCore, AppPaths};
pub use capabilities::{
    CapabilitySupport, RuntimeCapabilityDescriptor, all_runtime_capabilities, runtime_capabilities,
};
pub use commands::{Command, CommandOutcome};
pub use context::{ContextDoctorReport, RepoContext};
pub use diagnostics::{Diagnostic, DiagnosticSeverity};
pub use error::{AwoError, AwoResult};
pub use events::DomainEvent;
pub use routing::{
    RoutingContext, RoutingDecision, RoutingPreferences, RoutingRecommendation, RoutingSource,
    RoutingTarget, RuntimePressure, route_runtime,
};
pub use runtime::{RuntimeKind, SessionLaunchMode};
pub use skills::{
    RepoSkillCatalog, SkillDoctorReport, SkillLinkMode, SkillLinkReport, SkillRuntime,
};
pub use slot::SlotStrategy;
pub use snapshot::{
    AppSnapshot, CommandLogEntry, MemberRoutingSummary, RepoContextPackSummary,
    RepoSkillRuntimeSummary, RepoSummary, ReviewSummary, ReviewWarning,
    RoutingPreferencesSummary, SessionSummary, SlotSummary, TeamSummary,
};
pub use team::{
    TaskCard, TaskCardState, TeamExecutionMode, TeamManifest, TeamMember, TeamResetSummary,
    TeamStatus, TeamTaskExecution, TeamTaskStartOptions, TeamTeardownPlan, TeamTeardownResult,
    default_team_manifest_path, list_team_manifest_paths, load_team_manifest, remove_team_manifest,
    save_team_manifest, starter_team_manifest,
};
