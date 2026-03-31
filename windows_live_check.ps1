$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$stateRoot = "C:\tmp\awo-state-report"
$smokeRepo = "C:\tmp\awo-smoke-repo-report"
$resultPath = Join-Path $repoRoot "windows_checklist_live_results.json"

$env:PATH = "C:\Users\bismailov\.cargo\bin;C:\Program Files\Git\cmd;C:\Program Files\PowerShell\7;$env:PATH"

Remove-Item $stateRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $smokeRepo -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$stateRoot\config", "$stateRoot\data", "$stateRoot\clones", "$stateRoot\worktrees" | Out-Null

$baseEnv = @{
    "PATH" = $env:PATH
    "AWO_CONFIG_DIR" = "$stateRoot\config"
    "AWO_DATA_DIR" = "$stateRoot\data"
    "AWO_CLONES_DIR" = "$stateRoot\clones"
    "AWO_WORKTREES_DIR" = "$stateRoot\worktrees"
}

$results = New-Object System.Collections.Generic.List[object]

function Read-FileOrEmpty {
    param([string]$Path)

    if (Test-Path $Path) {
        return Get-Content -Raw $Path -ErrorAction SilentlyContinue
    }

    return ""
}

function Invoke-Proc {
    param(
        [string]$File,
        [string[]]$Args,
        [string]$WorkingDirectory
    )

    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $File
    foreach ($arg in $Args) {
        [void]$psi.ArgumentList.Add($arg)
    }
    $psi.WorkingDirectory = $WorkingDirectory
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false

    foreach ($kv in $baseEnv.GetEnumerator()) {
        $psi.Environment[$kv.Key] = $kv.Value
    }

    $process = [System.Diagnostics.Process]::Start($psi)
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()

    [pscustomobject]@{
        stdout = $stdout.TrimEnd()
        stderr = $stderr.TrimEnd()
        exit_code = $process.ExitCode
    }
}

function Run-Step {
    param(
        [string]$Name,
        [string]$File,
        [string[]]$Args,
        [string]$WorkingDirectory = $repoRoot
    )

    $res = Invoke-Proc -File $File -Args $Args -WorkingDirectory $WorkingDirectory
    $results.Add([pscustomobject]@{
        name = $Name
        file = $File
        args = $Args
        command = ((@($File) + $Args) -join " ")
        exit_code = $res.exit_code
        stdout = $res.stdout
        stderr = $res.stderr
    }) | Out-Null
    return $res
}

function Get-JsonFirstDataId {
    param(
        [pscustomobject]$Result,
        [string]$StepName
    )

    if ([string]::IsNullOrWhiteSpace($Result.stdout)) {
        throw "$StepName returned empty stdout. stderr: $($Result.stderr)"
    }

    $parsed = $Result.stdout | ConvertFrom-Json
    if ($null -eq $parsed.data) {
        throw "$StepName JSON payload had null data. stdout: $($Result.stdout)`nstderr: $($Result.stderr)"
    }

    if ($parsed.data -is [System.Array]) {
        if ($parsed.data.Count -lt 1) {
            throw "$StepName JSON payload had an empty data array. stdout: $($Result.stdout)`nstderr: $($Result.stderr)"
        }
        return $parsed.data[0].id
    }

    if ($null -ne $parsed.data.id) {
        return $parsed.data.id
    }

    throw "$StepName JSON payload did not contain a usable id. stdout: $($Result.stdout)`nstderr: $($Result.stderr)"
}

Run-Step "versions" "powershell.exe" @(
    "-NoLogo",
    "-NoProfile",
    "-Command",
    "git --version; cargo --version; rustc --version; pwsh --version; where.exe codex; where.exe claude; where.exe gemini"
) | Out-Null

Run-Step "repo_root_status_before" "C:\Program Files\Git\cmd\git.exe" @("status", "--short") | Out-Null
Run-Step "cargo_fmt" "cargo" @("fmt", "--all", "--check") | Out-Null
Run-Step "cargo_clippy" "cargo" @("clippy", "--all-targets", "--", "-D", "warnings") | Out-Null
Run-Step "cargo_test" "cargo" @("test", "-q", "--", "--test-threads=1") | Out-Null
Run-Step "cargo_build" "cargo" @("build") | Out-Null

Run-Step "binaries_awo" "powershell.exe" @(
    "-NoLogo",
    "-NoProfile",
    "-Command",
    "Get-ChildItem .\target\debug\awo.exe | Select-Object FullName,Length,LastWriteTime | ConvertTo-Json -Compress"
) | Out-Null
Run-Step "binaries_awod" "powershell.exe" @(
    "-NoLogo",
    "-NoProfile",
    "-Command",
    "Get-ChildItem .\target\debug\awod.exe | Select-Object FullName,Length,LastWriteTime | ConvertTo-Json -Compress"
) | Out-Null
Run-Step "binaries_awo_mcp" "powershell.exe" @(
    "-NoLogo",
    "-NoProfile",
    "-Command",
    "Get-ChildItem .\target\debug\awo-mcp.exe | Select-Object FullName,Length,LastWriteTime | ConvertTo-Json -Compress"
) | Out-Null

New-Item -ItemType Directory -Force $smokeRepo | Out-Null
Run-Step "smoke_repo_init" "powershell.exe" @(
    "-NoLogo",
    "-NoProfile",
    "-Command",
    "Set-Location '$smokeRepo'; git init -b main; '# Awo Smoke Repo' | Out-File -Encoding utf8 README.md; git add README.md; git -c user.name='Awo Smoke' -c user.email='smoke@example.com' commit -m 'init'"
) | Out-Null

$repoAdd = Run-Step "repo_add" (Join-Path $repoRoot "target\debug\awo.exe") @("--json", "repo", "add", $smokeRepo)
$repoId = Get-JsonFirstDataId -Result $repoAdd -StepName "repo_add"
Run-Step "repo_list" (Join-Path $repoRoot "target\debug\awo.exe") @("repo", "list") | Out-Null
Run-Step "context_pack" (Join-Path $repoRoot "target\debug\awo.exe") @("context", "pack", $repoId) | Out-Null
Run-Step "context_doctor" (Join-Path $repoRoot "target\debug\awo.exe") @("context", "doctor", $repoId) | Out-Null
Run-Step "skills_list" (Join-Path $repoRoot "target\debug\awo.exe") @("skills", "list", $repoId) | Out-Null
Run-Step "skills_doctor" (Join-Path $repoRoot "target\debug\awo.exe") @("skills", "doctor", $repoId) | Out-Null
Run-Step "runtime_list" (Join-Path $repoRoot "target\debug\awo.exe") @("runtime", "list") | Out-Null
Run-Step "runtime_show_claude" (Join-Path $repoRoot "target\debug\awo.exe") @("runtime", "show", "claude") | Out-Null
Run-Step "runtime_show_codex" (Join-Path $repoRoot "target\debug\awo.exe") @("runtime", "show", "codex") | Out-Null
Run-Step "runtime_show_gemini" (Join-Path $repoRoot "target\debug\awo.exe") @("runtime", "show", "gemini") | Out-Null
Run-Step "runtime_show_shell" (Join-Path $repoRoot "target\debug\awo.exe") @("runtime", "show", "shell") | Out-Null

$slotAcquire1 = Run-Step "slot_acquire_warm" (Join-Path $repoRoot "target\debug\awo.exe") @("--json", "slot", "acquire", $repoId, "warm-smoke", "--strategy", "warm")
$slot1 = Get-JsonFirstDataId -Result $slotAcquire1 -StepName "slot_acquire_warm"
Run-Step "slot_list_after_acquire1" (Join-Path $repoRoot "target\debug\awo.exe") @("slot", "list") | Out-Null
Run-Step "slot_release_1" (Join-Path $repoRoot "target\debug\awo.exe") @("slot", "release", $slot1) | Out-Null
Run-Step "slot_list_after_release1" (Join-Path $repoRoot "target\debug\awo.exe") @("slot", "list") | Out-Null
Run-Step "slot_delete_1" (Join-Path $repoRoot "target\debug\awo.exe") @("slot", "delete", $slot1) | Out-Null
Run-Step "slot_list_after_delete1" (Join-Path $repoRoot "target\debug\awo.exe") @("slot", "list") | Out-Null

$slotAcquire2 = Run-Step "slot_acquire_session" (Join-Path $repoRoot "target\debug\awo.exe") @("--json", "slot", "acquire", $repoId, "session-smoke", "--strategy", "warm")
$slot2 = Get-JsonFirstDataId -Result $slotAcquire2 -StepName "slot_acquire_session"
$sessionStart = Run-Step "session_start_shell" (Join-Path $repoRoot "target\debug\awo.exe") @("--json", "session", "start", $slot2, "shell", "echo 'hello from awo windows smoke'; pwd", "--read-only")
$sessionId = Get-JsonFirstDataId -Result $sessionStart -StepName "session_start_shell"
Run-Step "session_list" (Join-Path $repoRoot "target\debug\awo.exe") @("session", "list") | Out-Null
Run-Step "session_log" (Join-Path $repoRoot "target\debug\awo.exe") @("session", "log", $sessionId) | Out-Null
Run-Step "slot_release_2" (Join-Path $repoRoot "target\debug\awo.exe") @("slot", "release", $slot2) | Out-Null
Run-Step "slot_delete_2" (Join-Path $repoRoot "target\debug\awo.exe") @("slot", "delete", $slot2) | Out-Null

Run-Step "daemon_status_before" (Join-Path $repoRoot "target\debug\awo.exe") @("daemon", "status") | Out-Null
$stdoutLog = Join-Path $stateRoot "awod.stdout.log"
$stderrLog = Join-Path $stateRoot "awod.stderr.log"
Remove-Item $stdoutLog, $stderrLog -Force -ErrorAction SilentlyContinue
$awod = Start-Process -FilePath (Join-Path $repoRoot "target\debug\awod.exe") -WorkingDirectory $repoRoot -RedirectStandardOutput $stdoutLog -RedirectStandardError $stderrLog -PassThru
Start-Sleep -Seconds 2
Run-Step "daemon_status_running_1" (Join-Path $repoRoot "target\debug\awo.exe") @("daemon", "status") | Out-Null
Start-Sleep -Seconds 1
Run-Step "daemon_status_running_2" (Join-Path $repoRoot "target\debug\awo.exe") @("daemon", "status") | Out-Null
Run-Step "daemon_repo_list" (Join-Path $repoRoot "target\debug\awo.exe") @("repo", "list") | Out-Null
Run-Step "daemon_slot_list" (Join-Path $repoRoot "target\debug\awo.exe") @("slot", "list") | Out-Null
Run-Step "daemon_stop" (Join-Path $repoRoot "target\debug\awo.exe") @("daemon", "stop") | Out-Null
Start-Sleep -Seconds 1
Run-Step "daemon_status_after" (Join-Path $repoRoot "target\debug\awo.exe") @("daemon", "status") | Out-Null
$awod.Refresh()
$results.Add([pscustomobject]@{
    name = "daemon_process_after_stop"
    file = "powershell"
    args = @()
    command = "awod process state after stop"
    exit_code = $(if ($awod.HasExited) { 0 } else { 1 })
    stdout = $(if ($awod.HasExited) { "exited:$($awod.ExitCode)" } else { "still running" })
    stderr = (Read-FileOrEmpty $stderrLog)
}) | Out-Null
if (-not $awod.HasExited) {
    Stop-Process -Id $awod.Id -Force -ErrorAction SilentlyContinue
}

Run-Step "team_init" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "init", $repoId, "smoke-team", "Smoke the local orchestration loop") | Out-Null
Run-Step "team_member_add" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "member", "add", "smoke-team", "worker-a", "worker", "--runtime", "shell", "--model", "local-shell", "--notes", "smoke worker") | Out-Null
Run-Step "team_plan_add" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "plan", "add", "smoke-team", "plan-smoke", "Plan a shell task", "Create an executable task card from planning", "--owner-id", "worker-a", "--deliverable", "A generated task card") | Out-Null
Run-Step "team_plan_approve" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "plan", "approve", "smoke-team", "plan-smoke") | Out-Null
Run-Step "team_plan_generate" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "plan", "generate", "smoke-team", "plan-smoke", "task-planned", "--owner-id", "worker-a", "--deliverable", "A generated task card") | Out-Null
Run-Step "team_task_add" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "task", "add", "smoke-team", "task-shell", "worker-a", "Inspect repo", "pwd && ls", "--deliverable", "A repo listing", "--read-only") | Out-Null
Run-Step "team_task_start" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "task", "start", "smoke-team", "task-shell") | Out-Null
Run-Step "team_show_after_start" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "show", "smoke-team") | Out-Null
Run-Step "team_task_add_replacement" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "task", "add", "smoke-team", "task-replacement", "worker-a", "Replacement", "pwd", "--deliverable", "A replacement task", "--read-only") | Out-Null
Run-Step "team_task_supersede" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "task", "supersede", "smoke-team", "task-planned", "task-replacement") | Out-Null
Run-Step "team_show_after_supersede" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "show", "smoke-team") | Out-Null
Run-Step "team_report" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "report", "smoke-team") | Out-Null
Run-Step "team_teardown" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "teardown", "smoke-team", "--force") | Out-Null
Run-Step "team_delete" (Join-Path $repoRoot "target\debug\awo.exe") @("team", "delete", "smoke-team") | Out-Null

$quotedAwo = '"' + (Join-Path $repoRoot "target\debug\awo.exe") + '"'
Run-Step "tui_quit_smoke" "cmd.exe" @("/c", "echo q| $quotedAwo") | Out-Null

$results | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $resultPath
Write-Output $resultPath
