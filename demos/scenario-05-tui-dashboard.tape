# Scenario 5: TUI Dashboard — Interactive Control
# Demonstrates: TUI launch, panel navigation, Team Dashboard,
#               keybindings (Tab, j/k, T, /, ?, Esc, q)

Output demos/scenario-05-tui-dashboard.gif
Output demos/scenario-05-tui-dashboard.mp4

Require awo

Set Shell "zsh"
Set FontSize 16
Set FontFamily "JetBrains Mono"
Set Width 1400
Set Height 800
Set TypingSpeed 40ms
Set PlaybackSpeed 1.0
Set Padding 10
Set WindowBar Colorful
Set Theme "Catppuccin Mocha"

# ── Title card ─────────────────────────────────────────────
Type "# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
Enter
Type "# Scenario 5: TUI Dashboard — Interactive Control"
Enter
Type "# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
Enter
Sleep 2s

# ── Setup: populate state for the TUI ─────────────────────
Type "# Setting up state for the TUI to display..."
Enter
Sleep 500ms

Type "awo repo add ."
Enter
Sleep 2s

Type "awo team init chaban-04e494dd2c05 tui-demo 'Demonstrate TUI features'"
Enter
Sleep 2s

Type "awo team member add tui-demo worker-1 worker --runtime shell"
Enter
Sleep 1500ms

Type "awo team task add tui-demo build-check worker-1 'Build check' 'cargo build' --deliverable 'Binary'"
Enter
Sleep 1500ms

Type "awo team task add tui-demo test-run worker-1 'Run tests' 'cargo test' --deliverable 'Report' --depends-on build-check"
Enter
Sleep 1500ms

Type "awo slot acquire chaban-04e494dd2c05 manual-slot"
Enter
Sleep 2s

Type "# Setup complete. Launching the TUI..."
Enter
Sleep 1500ms

# ── Launch the TUI ─────────────────────────────────────────
Type "awo"
Enter
Sleep 4s

# ── [Tab] Navigate between panels ──────────────────────────
# Starting on Repos panel. Tab cycles: Repos -> Teams -> Slots -> Sessions

Tab
Sleep 2s

Tab
Sleep 2s

Tab
Sleep 2s

Tab
Sleep 2s

# ── [j/k] Navigate within a list ──────────────────────────
Type "j"
Sleep 1s
Type "j"
Sleep 1s
Type "k"
Sleep 1s

# ── [?] Open the help overlay ─────────────────────────────
Type "?"
Sleep 3s

Escape
Sleep 2s

# ── [T] Open the Team Dashboard ───────────────────────────
Type "T"
Sleep 3s

# [Tab] switches focus between team list and task list
Tab
Sleep 2s

# Navigate tasks
Type "j"
Sleep 1s
Type "k"
Sleep 1s

# Back to team list
Tab
Sleep 2s

# ── [Esc] Leave Team Dashboard ─────────────────────────────
Escape
Sleep 2s

# ── [/] Filter mode ───────────────────────────────────────
Type "/"
Sleep 1s

Type "manual"
Sleep 2s

# [Esc] clears the filter
Escape
Sleep 2s

# ── [r] Refresh data ──────────────────────────────────────
Type "r"
Sleep 2s

# ── [q] Quit the TUI ──────────────────────────────────────
Type "q"
Sleep 2s

# ── Cleanup ────────────────────────────────────────────────
Type "# Cleaning up..."
Enter
Sleep 500ms

Type "awo team teardown tui-demo --force"
Enter
Sleep 2s

Type "awo team delete tui-demo"
Enter
Sleep 1500ms

# Release the manual slot
Hide
Type "bash demos/cleanup-manual-slot.sh"
Enter
Sleep 3s
Show

Type "# Done! TUI walkthrough complete."
Enter
Sleep 3s
