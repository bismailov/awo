# Scenario 1: First Contact — Register a Repo and Explore
# Demonstrates: repo add, repo list, context pack, context doctor,
#               skills list, skills doctor, review status

Output demos/scenario-01-first-contact.gif
Output demos/scenario-01-first-contact.mp4

Require awo

Set Shell "zsh"
Set FontSize 18
Set FontFamily "JetBrains Mono"
Set Width 1400
Set Height 750
Set TypingSpeed 40ms
Set PlaybackSpeed 1.0
Set Padding 20
Set WindowBar Colorful
Set Theme "Catppuccin Mocha"

# ── Title card ──────────────────────────────────────────────
Type "# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
Enter
Type "# Scenario 1: First Contact — Register a Repo and Explore"
Enter
Type "# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
Enter
Sleep 2s

# ── Step 1: Register this repo ─────────────────────────────
Type "# Step 1: Register the current Git repo with awo"
Enter
Sleep 500ms

Type "awo repo add ."
Sleep 500ms
Enter
Sleep 3s

# ── Step 2: List registered repos ──────────────────────────
Type "# Step 2: List registered repos"
Enter
Sleep 500ms

Type "awo repo list"
Sleep 500ms
Enter
Sleep 3s

# ── Step 3: Inspect context ────────────────────────────────
Type "# Step 3: What files does awo consider important?"
Enter
Sleep 500ms

Type "awo context pack chaban-04e494dd2c05"
Sleep 500ms
Enter
Sleep 4s

# ── Step 4: Context doctor ─────────────────────────────────
Type "# Step 4: Run the context doctor — any warnings?"
Enter
Sleep 500ms

Type "awo context doctor chaban-04e494dd2c05"
Sleep 500ms
Enter
Sleep 3s

# ── Step 5: List skills ───────────────────────────────────
Type "# Step 5: Check available skills"
Enter
Sleep 500ms

Type "awo skills list chaban-04e494dd2c05"
Sleep 500ms
Enter
Sleep 3s

# ── Step 6: Skills doctor ─────────────────────────────────
Type "# Step 6: Run skills doctor"
Enter
Sleep 500ms

Type "awo skills doctor chaban-04e494dd2c05"
Sleep 500ms
Enter
Sleep 3s

# ── Step 7: Review status ─────────────────────────────────
Type "# Step 7: Check review status (should be clean — no active slots)"
Enter
Sleep 500ms

Type "awo review status"
Sleep 500ms
Enter
Sleep 3s

# ── Done ──────────────────────────────────────────────────
Type "# Done! The repo is registered and awo knows its structure."
Enter
Sleep 3s
