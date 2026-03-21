use std::process::Command;

pub fn executable_exists(name: &str) -> bool {
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
        if executable_exists("pwsh") {
            "pwsh"
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
