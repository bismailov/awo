use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AwoError {
    #[error("unknown repo id `{repo_id}`")]
    UnknownRepoId { repo_id: String },
    #[error("unknown slot id `{slot_id}`")]
    UnknownSlotId { slot_id: String },
    #[error("unknown session id `{session_id}`")]
    UnknownSessionId { session_id: String },
    #[error("unknown task `{task_id}`")]
    UnknownTaskId { task_id: String },
    #[error("unknown owner `{owner_id}`")]
    UnknownOwnerId { owner_id: String },
    #[error("unsupported value `{value}` for {kind}")]
    UnsupportedValue { kind: &'static str, value: String },
    #[error("invalid state: {message}")]
    InvalidState { message: String },
    #[error("failed to resolve application directories")]
    ProjectDirectoriesUnavailable,
    #[error("io error while {action} at {path}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to acquire {mode} lock at {path}")]
    FileLock {
        mode: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse team manifest at {path}")]
    TeamManifestParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to serialize team manifest")]
    TeamManifestSerialize {
        #[source]
        source: toml::ser::Error,
    },
    #[error("failed to run git {operation} in {path}")]
    GitInvocation {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("git {operation} failed in {path}: {message}")]
    GitCommandFailed {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type AwoResult<T> = std::result::Result<T, AwoError>;

impl AwoError {
    pub fn unknown_repo(repo_id: impl Into<String>) -> Self {
        Self::UnknownRepoId {
            repo_id: repo_id.into(),
        }
    }

    pub fn unknown_slot(slot_id: impl Into<String>) -> Self {
        Self::UnknownSlotId {
            slot_id: slot_id.into(),
        }
    }

    pub fn unknown_session(session_id: impl Into<String>) -> Self {
        Self::UnknownSessionId {
            session_id: session_id.into(),
        }
    }

    pub fn unknown_task(task_id: impl Into<String>) -> Self {
        Self::UnknownTaskId {
            task_id: task_id.into(),
        }
    }

    pub fn unknown_owner(owner_id: impl Into<String>) -> Self {
        Self::UnknownOwnerId {
            owner_id: owner_id.into(),
        }
    }

    pub fn unsupported(kind: &'static str, value: impl Into<String>) -> Self {
        Self::UnsupportedValue {
            kind,
            value: value.into(),
        }
    }

    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::InvalidState {
            message: message.into(),
        }
    }

    pub fn project_directories_unavailable() -> Self {
        Self::ProjectDirectoriesUnavailable
    }

    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }

    pub fn file_lock(mode: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::FileLock {
            mode,
            path: path.into(),
            source,
        }
    }

    pub fn team_manifest_parse(path: impl Into<PathBuf>, source: toml::de::Error) -> Self {
        Self::TeamManifestParse {
            path: path.into(),
            source,
        }
    }

    pub fn team_manifest_serialize(source: toml::ser::Error) -> Self {
        Self::TeamManifestSerialize { source }
    }

    pub fn git_invocation(
        operation: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::GitInvocation {
            operation,
            path: path.into(),
            source,
        }
    }

    pub fn git_command_failed(
        operation: &'static str,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self::GitCommandFailed {
            operation,
            path: path.into(),
            message: message.into(),
        }
    }
}
