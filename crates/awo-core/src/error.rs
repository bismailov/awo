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
}
