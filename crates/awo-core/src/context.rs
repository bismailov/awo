use crate::diagnostics::Diagnostic;
use crate::error::{AwoError, AwoResult};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ContextFile {
    pub label: String,
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPack {
    pub name: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoContext {
    pub repo_root: String,
    pub entrypoints: Vec<ContextFile>,
    pub standards: Vec<ContextFile>,
    pub docs: Vec<ContextFile>,
    pub analysis_files: Vec<ContextFile>,
    pub packs: Vec<ContextPack>,
    pub mcp_config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextDoctorReport {
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionContextPlan {
    pub mandatory_files: Vec<String>,
    pub selected_packs: Vec<ContextPack>,
    pub notes: Vec<String>,
}

impl SessionContextPlan {
    pub fn total_file_count(&self) -> usize {
        self.mandatory_files.len()
            + self
                .selected_packs
                .iter()
                .map(|pack| pack.files.len())
                .sum::<usize>()
    }

    pub fn selected_pack_names(&self) -> Vec<String> {
        self.selected_packs
            .iter()
            .map(|pack| pack.name.clone())
            .collect()
    }
}

pub fn discover_repo_context(repo_root: &Path) -> AwoResult<RepoContext> {
    let repo_root_display = repo_root.display().to_string();
    let repo_root = dunce::canonicalize(repo_root)
        .map_err(|source| AwoError::io("canonicalize repo root", repo_root_display, source))?;

    let entrypoints = discover_entrypoints(&repo_root);
    let standards = discover_standard_files(&repo_root);
    let standard_paths = standards
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();

    let docs_dir = repo_root.join("docs");
    let docs = collect_markdown_files(&docs_dir)?
        .into_iter()
        .filter(|path| !standard_paths.contains(&path.display().to_string()))
        .map(|path| to_context_file(path, "documentation"))
        .collect::<Vec<_>>();

    let analysis_dir = repo_root.join("analysis");
    let analysis_files = collect_markdown_files(&analysis_dir)?
        .into_iter()
        .map(|path| to_context_file(path, "analysis"))
        .collect::<Vec<_>>();
    let packs = build_analysis_packs(&analysis_files);

    let mcp_config_path = repo_root
        .join(".mcp.json")
        .exists()
        .then(|| repo_root.join(".mcp.json").display().to_string());

    Ok(RepoContext {
        repo_root: repo_root.display().to_string(),
        entrypoints,
        standards,
        docs,
        analysis_files,
        packs,
        mcp_config_path,
    })
}

pub fn doctor_repo_context(context: &RepoContext) -> ContextDoctorReport {
    let mut diagnostics = Vec::new();
    let entrypoint_names = context
        .entrypoints
        .iter()
        .map(|file| file.label.as_str())
        .collect::<BTreeSet<_>>();

    if context.entrypoints.is_empty() {
        diagnostics.push(Diagnostic::error(
            "context.entrypoints.missing",
            "No repo entrypoint files were detected. Add `AGENTS.md` or `PROJECT.md` at the repo root.",
        ));
    } else {
        diagnostics.push(Diagnostic::info(
            "context.entrypoints.present",
            format!(
                "Detected {} repo entrypoint file(s).",
                context.entrypoints.len()
            ),
        ));
    }

    if entrypoint_names.contains("PROJECT.md") && !entrypoint_names.contains("AGENTS.md") {
        diagnostics.push(Diagnostic::warning(
            "context.agents-wrapper.missing",
            "Detected `PROJECT.md` without `AGENTS.md`. A thin `AGENTS.md` wrapper improves cross-agent compatibility.",
        ));
    }

    if (entrypoint_names.contains("CLAUDE.md") || entrypoint_names.contains("GEMINI.md"))
        && !entrypoint_names.contains("PROJECT.md")
    {
        diagnostics.push(Diagnostic::warning(
            "context.neutral-brain.missing",
            "Vendor-specific wrappers were detected without `PROJECT.md`. Consider adding a neutral project-brain file.",
        ));
    }

    if context.standards.is_empty() {
        diagnostics.push(Diagnostic::warning(
            "context.standards.missing",
            "No standards documents were detected. Add repo-specific guidance such as `docs/agentic-coding.md` or `docs/testing.md`.",
        ));
    } else {
        diagnostics.push(Diagnostic::info(
            "context.standards.present",
            format!(
                "Detected {} standards document(s).",
                context.standards.len()
            ),
        ));
    }

    if context.mcp_config_path.is_none() {
        diagnostics.push(Diagnostic::info(
            "context.mcp.absent",
            "No project `.mcp.json` was detected.",
        ));
    }

    if context.analysis_files.is_empty() {
        diagnostics.push(Diagnostic::info(
            "context.analysis.absent",
            "No `analysis/` library was detected for optional audit or architecture context.",
        ));
    } else {
        diagnostics.push(Diagnostic::info(
            "context.analysis.present",
            format!(
                "Detected {} analysis file(s) grouped into {} pack(s).",
                context.analysis_files.len(),
                context.packs.len()
            ),
        ));
    }

    ContextDoctorReport { diagnostics }
}

pub fn plan_session_context(context: &RepoContext, prompt: &str) -> SessionContextPlan {
    let mut mandatory_files = context
        .entrypoints
        .iter()
        .map(|file| file.path.clone())
        .chain(context.standards.iter().map(|file| file.path.clone()))
        .collect::<Vec<_>>();

    if let Some(mcp_config_path) = &context.mcp_config_path {
        mandatory_files.push(mcp_config_path.clone());
    }
    mandatory_files.sort();
    mandatory_files.dedup();

    let selected_pack_names = select_analysis_packs(context, prompt);
    let selected_packs = context
        .packs
        .iter()
        .filter(|pack| selected_pack_names.contains(&pack.name))
        .cloned()
        .collect::<Vec<_>>();

    let mut notes = Vec::new();
    if Path::new(&context.repo_root)
        .join(".agents/skills")
        .exists()
    {
        notes.push(
            "Shared repo skills live under `.agents/skills`; prefer repo-local skills when the runtime also has global copies."
                .to_string(),
        );
    }
    if !selected_packs.is_empty() {
        notes.push(
            "Use the selected analysis pack files as optional deep context, not as mandatory startup reading."
                .to_string(),
        );
    }

    SessionContextPlan {
        mandatory_files,
        selected_packs,
        notes,
    }
}

pub fn render_session_context_prompt(
    context: &RepoContext,
    plan: &SessionContextPlan,
    user_prompt: &str,
) -> String {
    let mut sections = vec![
        "Repository launch context:".to_string(),
        format!("- repo root: {}", context.repo_root),
        "- Start by reading these shared repo files before making changes:".to_string(),
    ];

    for file in &plan.mandatory_files {
        sections.push(format!("  - {file}"));
    }

    if plan.selected_packs.is_empty() {
        sections.push("- No analysis packs were auto-selected for this task.".to_string());
    } else {
        sections.push("- Additional analysis packs selected for this task:".to_string());
        for pack in &plan.selected_packs {
            sections.push(format!("  - {} ({} files)", pack.name, pack.files.len()));
            for file in &pack.files {
                sections.push(format!("    - {file}"));
            }
        }
    }

    if !plan.notes.is_empty() {
        sections.push("- Additional orchestration notes:".to_string());
        for note in &plan.notes {
            sections.push(format!("  - {note}"));
        }
    }

    sections.push(String::new());
    sections.push("User task:".to_string());
    sections.push(user_prompt.to_string());
    sections.join("\n")
}

fn discover_entrypoints(repo_root: &Path) -> Vec<ContextFile> {
    [
        "AGENTS.md",
        "PROJECT.md",
        "CLAUDE.md",
        "GEMINI.md",
        "README.md",
    ]
    .into_iter()
    .map(|name| repo_root.join(name))
    .filter(|path| path.exists())
    .map(|path| to_context_file(path, "entrypoint"))
    .collect()
}

fn discover_standard_files(repo_root: &Path) -> Vec<ContextFile> {
    [
        "docs/agentic-coding.md",
        "docs/multi-model-context-strategy.md",
        "docs/ui-design-system.md",
        "docs/testing.md",
        "docs/deployment.md",
    ]
    .into_iter()
    .map(|relative| repo_root.join(relative))
    .filter(|path| path.exists())
    .map(|path| to_context_file(path, "standard"))
    .collect()
}

fn collect_markdown_files(dir: &Path) -> AwoResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(dir).map_err(|source| AwoError::io("read directory", dir, source))? {
        let entry = entry.map_err(|source| AwoError::io("read directory entry", dir, source))?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_markdown_files(&path)?);
        } else if path.extension().and_then(|value| value.to_str()) == Some("md") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn build_analysis_packs(files: &[ContextFile]) -> Vec<ContextPack> {
    let mut packs = BTreeMap::<String, Vec<String>>::new();
    for file in files {
        let pack_name = classify_analysis_pack(&file.label).to_string();
        packs.entry(pack_name).or_default().push(file.path.clone());
    }

    packs
        .into_iter()
        .map(|(name, mut files)| {
            files.sort();
            ContextPack { name, files }
        })
        .collect()
}

fn select_analysis_packs(context: &RepoContext, prompt: &str) -> BTreeSet<String> {
    let lower = prompt.to_ascii_lowercase();
    let mut selected = BTreeSet::new();

    let audit_keywords = [
        "audit",
        "review",
        "bug",
        "incident",
        "issue",
        "regression",
        "qa",
        "performance",
        "test",
        "verify",
    ];
    let architecture_keywords = [
        "architecture",
        "design",
        "plan",
        "understand",
        "investigate",
        "explore",
        "middleware",
        "orchestr",
    ];
    let refactor_keywords = ["refactor", "cleanup", "rewrite", "simplify", "extract"];

    if audit_keywords.iter().any(|needle| lower.contains(needle)) {
        selected.insert("audit".to_string());
    }
    if architecture_keywords
        .iter()
        .any(|needle| lower.contains(needle))
    {
        selected.insert("architecture".to_string());
    }
    if refactor_keywords
        .iter()
        .any(|needle| lower.contains(needle))
    {
        selected.insert("refactor".to_string());
    }

    if selected.is_empty() && !context.packs.is_empty() {
        if context.packs.iter().any(|pack| pack.name == "architecture") {
            selected.insert("architecture".to_string());
        } else if let Some(first_pack) = context.packs.first() {
            selected.insert(first_pack.name.clone());
        }
    }

    selected
}

fn classify_analysis_pack(file_name: &str) -> &'static str {
    let lower = file_name.to_ascii_lowercase();
    if ["audit", "qa", "remediation", "issue", "review"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        "audit"
    } else if ["offline", "architecture", "plan"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        "architecture"
    } else if lower.contains("refactor") {
        "refactor"
    } else {
        "analysis"
    }
}

fn to_context_file(path: PathBuf, kind: &str) -> ContextFile {
    ContextFile {
        label: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
            .to_string(),
        path: path.display().to_string(),
        kind: kind.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{discover_repo_context, doctor_repo_context};
    use anyhow::Result;
    use std::fs;

    #[test]
    fn discovers_entrypoints_and_analysis_packs() -> Result<()> {
        let repo = tempfile::tempdir()?;
        fs::create_dir_all(repo.path().join("docs"))?;
        fs::create_dir_all(repo.path().join("analysis"))?;
        fs::write(repo.path().join("PROJECT.md"), "# Project\n")?;
        fs::write(repo.path().join("AGENTS.md"), "Read PROJECT.md\n")?;
        fs::write(repo.path().join("docs/agentic-coding.md"), "# Guide\n")?;
        fs::write(repo.path().join("analysis/code_audit.md"), "# Audit\n")?;
        fs::write(
            repo.path().join("analysis/offline-implementation-plan.md"),
            "# Plan\n",
        )?;
        fs::write(repo.path().join(".mcp.json"), "{}\n")?;

        let context = discover_repo_context(repo.path())?;
        assert_eq!(context.entrypoints.len(), 2);
        assert_eq!(context.standards.len(), 1);
        assert_eq!(context.analysis_files.len(), 2);
        assert_eq!(context.packs.len(), 2);
        assert!(context.mcp_config_path.is_some());

        Ok(())
    }

    #[test]
    fn doctor_warns_when_agents_wrapper_is_missing() -> Result<()> {
        let repo = tempfile::tempdir()?;
        fs::write(repo.path().join("PROJECT.md"), "# Project\n")?;

        let context = discover_repo_context(repo.path())?;
        let report = doctor_repo_context(&context);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diag| diag.code == "context.agents-wrapper.missing")
        );

        Ok(())
    }
}
