pub mod app;
pub mod capabilities;
pub mod commands;
pub mod config;
pub mod context;
pub mod diagnostics;
pub mod events;
pub mod fingerprint;
pub mod git;
pub mod platform;
pub mod repo;
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
pub use events::DomainEvent;
pub use runtime::{RuntimeKind, SessionLaunchMode};
pub use skills::{
    RepoSkillCatalog, SkillDoctorReport, SkillLinkMode, SkillLinkReport, SkillRuntime,
};
pub use slot::SlotStrategy;
pub use snapshot::{
    AppSnapshot, CommandLogEntry, RepoContextPackSummary, RepoSkillRuntimeSummary, RepoSummary,
    ReviewSummary, ReviewWarning, SessionSummary, SlotSummary,
};
pub use team::{
    TaskCard, TaskCardState, TeamExecutionMode, TeamManifest, TeamMember, TeamStatus,
    default_team_manifest_path, list_team_manifest_paths, load_team_manifest, save_team_manifest,
};
