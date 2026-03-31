param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug",
    [string]$BinaryDir = "",
    [string]$ReportJson = "",
    [string]$ReportMarkdown = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$python = (Get-Command python -ErrorAction SilentlyContinue)
if ($null -eq $python) {
    $python = (Get-Command py -ErrorAction Stop)
}

if ([string]::IsNullOrWhiteSpace($ReportJson)) {
    $ReportJson = Join-Path $repoRoot "windows_checklist_live_results.json"
}

if ([string]::IsNullOrWhiteSpace($ReportMarkdown)) {
    $ReportMarkdown = Join-Path $repoRoot "windows_checklist_live_results.md"
}

$args = @(
    (Join-Path $repoRoot "scripts\awo_smoke.py"),
    "--profile", $Profile,
    "--report-json", $ReportJson,
    "--report-md", $ReportMarkdown
)

if (-not [string]::IsNullOrWhiteSpace($BinaryDir)) {
    $args += @("--binary-dir", $BinaryDir)
}

& $python.Source @args
