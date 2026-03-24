pub mod app;
pub mod capabilities;
pub mod commands;
pub mod config;
pub mod context;
pub mod daemon;
pub mod diagnostics;
pub mod dispatch;
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
#[cfg(unix)]
pub use daemon::DaemonClient;
pub use daemon::{
    DaemonOptions, DaemonServer, DaemonStatus, ShutdownHandle, daemon_is_running,
    get_daemon_status, spawn_daemon, stop_daemon,
};
pub use diagnostics::{Diagnostic, DiagnosticSeverity};
pub use dispatch::{
    Dispatcher, RpcError, RpcRequest, RpcResponse, RpcResult, dispatch_rpc, error_code_for,
    parse_rpc_request,
};
pub use error::{AwoError, AwoResult};
pub use events::{DomainEvent, EventBus, EventEntry, EventPollResult};
pub use routing::{
    RoutingContext, RoutingDecision, RoutingPreferences, RoutingRecommendation, RoutingSource,
    RoutingTarget, RuntimePressure, route_runtime,
};
pub use runtime::{RuntimeKind, SessionLaunchMode, SessionStatus};
pub use skills::{
    RepoSkillCatalog, SkillDoctorReport, SkillLinkMode, SkillLinkReport, SkillRuntime,
};
pub use slot::{FingerprintStatus, SlotStatus, SlotStrategy};
pub use snapshot::{
    AppSnapshot, CommandLogEntry, MemberRoutingSummary, RepoContextPackSummary,
    RepoSkillRuntimeSummary, RepoSummary, ReviewSummary, ReviewWarning, RoutingPreferencesSummary,
    SessionSummary, SlotSummary, TeamSummary,
};
pub use team::{
    TaskCard, TaskCardState, TeamExecutionMode, TeamManifest, TeamMember, TeamResetSummary,
    TeamStatus, TeamTaskExecution, TeamTaskStartOptions, TeamTeardownPlan, TeamTeardownResult,
    default_team_manifest_path, list_team_manifest_paths, load_team_manifest, remove_team_manifest,
    save_team_manifest, starter_team_manifest,
};
