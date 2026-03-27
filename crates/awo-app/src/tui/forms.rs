use awo_core::routing::RoutingPreferences;
use awo_core::team::TeamMember;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FormKind {
    RepoAdd,
    TeamInit,
    MemberAdd { team_id: String },
    MemberUpdate { team_id: String, member_id: String },
    TaskAdd { team_id: String },
    TaskDelegate { team_id: String, task_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Text,
    Choice(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormField {
    pub key: &'static str,
    pub label: String,
    pub value: String,
    pub kind: FieldKind,
}

impl FormField {
    pub fn text(key: &'static str, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
            value: value.into(),
            kind: FieldKind::Text,
        }
    }

    pub fn choice(
        key: &'static str,
        label: impl Into<String>,
        options: Vec<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            key,
            label: label.into(),
            value: value.into(),
            kind: FieldKind::Choice(options),
        }
    }

    pub fn cycle(&mut self, direction: i32) {
        let FieldKind::Choice(options) = &self.kind else {
            return;
        };
        if options.is_empty() {
            return;
        }

        let current = options
            .iter()
            .position(|option| option == &self.value)
            .unwrap_or(0);
        let len = options.len() as i32;
        let next = (current as i32 + direction).rem_euclid(len) as usize;
        self.value = options[next].clone();
    }

    pub fn is_choice(&self) -> bool {
        matches!(self.kind, FieldKind::Choice(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormState {
    pub kind: FormKind,
    pub title: String,
    pub submit_label: String,
    pub fields: Vec<FormField>,
    pub selected: usize,
    pub error: Option<String>,
    pub footer: Option<String>,
}

impl FormState {
    pub fn new(
        kind: FormKind,
        title: impl Into<String>,
        submit_label: impl Into<String>,
        fields: Vec<FormField>,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            submit_label: submit_label.into(),
            fields,
            selected: 0,
            error: None,
            footer: None,
        }
    }

    pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    pub fn next_field(&mut self) {
        if !self.fields.is_empty() {
            self.selected = (self.selected + 1) % self.fields.len();
        }
    }

    pub fn prev_field(&mut self) {
        if !self.fields.is_empty() {
            self.selected = (self.selected + self.fields.len() - 1) % self.fields.len();
        }
    }

    pub fn selected_field_mut(&mut self) -> Option<&mut FormField> {
        self.fields.get_mut(self.selected)
    }

    pub fn value(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|field| field.key == key)
            .map(|field| field.value.as_str())
    }

    pub fn repo_add(default_path: String) -> Self {
        Self::new(
            FormKind::RepoAdd,
            "Add Repository",
            "Register",
            vec![FormField::text("path", "Repository path", default_path)],
        )
    }

    pub fn team_init(repo_ids: Vec<String>, selected_repo_id: Option<String>) -> Self {
        let default_repo = selected_repo_id
            .filter(|repo_id| repo_ids.iter().any(|candidate| candidate == repo_id))
            .or_else(|| repo_ids.first().cloned())
            .unwrap_or_default();

        Self::new(
            FormKind::TeamInit,
            "Create Team",
            "Create",
            vec![
                FormField::choice("repo_id", "Repository", repo_ids, default_repo),
                FormField::text("team_id", "Team ID", ""),
                FormField::text("objective", "Objective", ""),
            ],
        )
        .with_footer("Defaults: external_slots lead with no runtime/model overrides")
    }

    pub fn member_add(team_id: String) -> Self {
        Self::new(
            FormKind::MemberAdd { team_id },
            "Add Member",
            "Add",
            vec![
                FormField::text("member_id", "Member ID", ""),
                FormField::choice(
                    "role",
                    "Role",
                    vec![
                        "worker".to_string(),
                        "implementer".to_string(),
                        "reviewer".to_string(),
                    ],
                    "worker",
                ),
                FormField::choice("runtime", "Runtime", runtime_options(), "codex"),
                FormField::text("model", "Model", ""),
                FormField::choice(
                    "read_only",
                    "Read-only",
                    vec!["false".to_string(), "true".to_string()],
                    "false",
                ),
            ],
        )
        .with_footer("Execution mode defaults to external_slots")
    }

    pub fn member_update(team_id: String, member: &TeamMember) -> Self {
        let preferences = member.routing_preferences.clone().unwrap_or_default();
        Self::new(
            FormKind::MemberUpdate {
                team_id,
                member_id: member.member_id.clone(),
            },
            format!("Update Member: {}", member.member_id),
            "Save",
            vec![
                FormField::choice(
                    "runtime",
                    "Runtime",
                    optional_runtime_options(),
                    member.runtime.clone().unwrap_or_default(),
                ),
                FormField::text("model", "Model", member.model.clone().unwrap_or_default()),
                FormField::choice(
                    "fallback_runtime",
                    "Fallback runtime",
                    optional_runtime_options(),
                    member.fallback_runtime.clone().unwrap_or_default(),
                ),
                FormField::text(
                    "fallback_model",
                    "Fallback model",
                    member.fallback_model.clone().unwrap_or_default(),
                ),
                FormField::choice(
                    "allow_fallback",
                    "Allow fallback",
                    vec!["true".to_string(), "false".to_string()],
                    if preferences.allow_fallback {
                        "true"
                    } else {
                        "false"
                    },
                ),
                FormField::choice(
                    "prefer_local",
                    "Prefer local",
                    vec!["false".to_string(), "true".to_string()],
                    if preferences.prefer_local {
                        "true"
                    } else {
                        "false"
                    },
                ),
                FormField::choice(
                    "avoid_metered",
                    "Avoid metered",
                    vec!["false".to_string(), "true".to_string()],
                    if preferences.avoid_metered {
                        "true"
                    } else {
                        "false"
                    },
                ),
                FormField::choice(
                    "max_cost_tier",
                    "Max cost tier",
                    vec![
                        "".to_string(),
                        "local".to_string(),
                        "cheap".to_string(),
                        "standard".to_string(),
                        "premium".to_string(),
                    ],
                    preferences
                        .max_cost_tier
                        .map(|tier| tier.as_str().to_string())
                        .unwrap_or_default(),
                ),
            ],
        )
    }

    pub fn task_add(team_id: String, owner_ids: Vec<String>) -> Self {
        let default_owner = owner_ids.first().cloned().unwrap_or_default();
        Self::new(
            FormKind::TaskAdd { team_id },
            "Add Task",
            "Add",
            vec![
                FormField::text("task_id", "Task ID", ""),
                FormField::choice("owner_id", "Owner", owner_ids, default_owner),
                FormField::text("title", "Title", ""),
                FormField::text("summary", "Summary", ""),
                FormField::choice(
                    "runtime",
                    "Runtime override",
                    optional_runtime_options(),
                    "",
                ),
                FormField::choice(
                    "read_only",
                    "Read-only",
                    vec!["false".to_string(), "true".to_string()],
                    "false",
                ),
                FormField::text("write_scope", "Write scope (comma-separated)", ""),
                FormField::text("deliverable", "Deliverable", ""),
                FormField::text("verification", "Verification (comma-separated)", ""),
                FormField::text("depends_on", "Depends on (comma-separated)", ""),
            ],
        )
    }

    pub fn task_delegate(team_id: String, task_id: String, target_member_ids: Vec<String>) -> Self {
        let default_target = target_member_ids.first().cloned().unwrap_or_default();
        Self::new(
            FormKind::TaskDelegate { team_id, task_id },
            "Delegate Task",
            "Delegate",
            vec![
                FormField::choice(
                    "target_member_id",
                    "Target member",
                    target_member_ids,
                    default_target,
                ),
                FormField::text("lead_notes", "Lead notes", ""),
                FormField::text("focus_files", "Focus files (comma-separated)", ""),
                FormField::choice(
                    "auto_start",
                    "Auto-start",
                    vec!["true".to_string(), "false".to_string()],
                    "true",
                ),
            ],
        )
        .with_footer("Delegation preserves immutable tasks by reassigning ownership, not editing task content")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfirmAction {
    RemoveMember { team_id: String, member_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfirmState {
    pub title: String,
    pub message: String,
    pub action: ConfirmAction,
}

impl ConfirmState {
    pub fn remove_member(team_id: String, member_id: String) -> Self {
        Self {
            title: "Remove Member".to_string(),
            message: format!(
                "Remove member `{member_id}` from team `{team_id}`?\nPress Enter to confirm or Esc to cancel."
            ),
            action: ConfirmAction::RemoveMember { team_id, member_id },
        }
    }
}

fn runtime_options() -> Vec<String> {
    vec![
        "codex".to_string(),
        "claude".to_string(),
        "gemini".to_string(),
        "shell".to_string(),
    ]
}

fn optional_runtime_options() -> Vec<String> {
    let mut options = vec![String::new()];
    options.extend(runtime_options());
    options
}

pub(crate) fn routing_preferences_from_form(form: &FormState) -> RoutingPreferences {
    RoutingPreferences {
        allow_fallback: form.value("allow_fallback") == Some("true"),
        prefer_local: form.value("prefer_local") == Some("true"),
        avoid_metered: form.value("avoid_metered") == Some("true"),
        max_cost_tier: form
            .value("max_cost_tier")
            .and_then(blank_to_none)
            .and_then(|value| value.parse().ok()),
    }
}

pub(crate) fn blank_to_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{FormField, FormState, split_csv};

    #[test]
    fn choice_field_cycles_in_both_directions() {
        let mut field = FormField::choice(
            "runtime",
            "Runtime",
            vec![
                "codex".to_string(),
                "claude".to_string(),
                "gemini".to_string(),
            ],
            "codex",
        );

        field.cycle(1);
        assert_eq!(field.value, "claude");

        field.cycle(-1);
        assert_eq!(field.value, "codex");

        field.cycle(-1);
        assert_eq!(field.value, "gemini");
    }

    #[test]
    fn team_init_prefills_selected_repo() {
        let form = FormState::team_init(
            vec!["repo-a".to_string(), "repo-b".to_string()],
            Some("repo-b".to_string()),
        );

        assert_eq!(form.value("repo_id"), Some("repo-b"));
    }

    #[test]
    fn split_csv_trims_and_ignores_blanks() {
        assert_eq!(
            split_csv(" src/lib.rs, , tests/foo.rs "),
            vec!["src/lib.rs".to_string(), "tests/foo.rs".to_string()]
        );
    }
}
