use std::path::Path;
use std::process::Command;

pub fn executable_exists(name: &str) -> bool {
    let candidate = Path::new(name);
    if candidate.is_absolute() || name.contains('\\') || name.contains('/') {
        return candidate.exists();
    }

    #[cfg(windows)]
    let probe = "where";
    #[cfg(not(windows))]
    let probe = "which";

    Command::new(probe)
        .arg(name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn default_shell_program() -> &'static str {
    #[cfg(windows)]
    {
        const PWSH_PATH: &str = r"C:\Program Files\PowerShell\7\pwsh.exe";
        const POWERSHELL_PATH: &str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";

        if executable_exists("pwsh") {
            "pwsh"
        } else if executable_exists(PWSH_PATH) {
            PWSH_PATH
        } else if executable_exists(POWERSHELL_PATH) {
            POWERSHELL_PATH
        } else {
            "powershell"
        }
    }

    #[cfg(not(windows))]
    {
        if executable_exists("zsh") {
            "zsh"
        } else if executable_exists("bash") {
            "bash"
        } else {
            "sh"
        }
    }
}

pub fn shell_command_args(command: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            command.to_string(),
        ]
    }

    #[cfg(not(windows))]
    {
        vec!["-lc".to_string(), command.to_string()]
    }
}

pub fn shell_script_args(script_path: &Path) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            script_path.display().to_string(),
        ]
    }

    #[cfg(not(windows))]
    {
        vec![script_path.display().to_string()]
    }
}

pub fn supports_tmux_supervision() -> bool {
    #[cfg(unix)]
    {
        executable_exists("tmux")
    }

    #[cfg(not(unix))]
    {
        false
    }
}

pub fn current_platform_label() -> &'static str {
    std::env::consts::OS
}
