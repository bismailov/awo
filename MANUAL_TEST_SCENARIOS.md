# Manual Test Scenarios for Awo Console

Hands-on scenarios to exercise the full feature set. Each scenario builds on the previous.

Prerequisites: build the project first.

```bash
cargo build
```

---

## Scenario 1: First Contact — Register a Repo and Explore

**Goal:** Register a real Git repo and explore what Awo Console sees.

```bash
# 1. Register this repo itself
./target/debug/awo repo add .

# 2. List registered repos (should show chaban)
./target/debug/awo repo list

# 3. Inspect context — what files does Awo Console consider important?
./target/debug/awo context pack <REPO_ID>

# 4. Run the context doctor — any warnings?
./target/debug/awo context doctor <REPO_ID>

# 5. Check available skills
./target/debug/awo skills list <REPO_ID>

# 6. Run skills doctor
./target/debug/awo skills doctor <REPO_ID>

# 7. Check review status (should be clean — no slots yet)
./target/debug/awo review status
```

Replace `<REPO_ID>` with the ID printed by `repo list` (e.g. `chaban-abc123`).

---

## Scenario 2: Solo Slot Lifecycle — Acquire, Use, Release

**Goal:** Create an isolated worktree slot, inspect it, release it.

```bash
# 1. Acquire a fresh slot for a task
./target/debug/awo slot acquire <REPO_ID> fix-readme

# 2. List slots — your new slot should appear
./target/debug/awo slot list

# 3. The slot created a real Git worktree. Inspect it:
ls <SLOT_PATH>          # path from the slot acquire output
cd <SLOT_PATH> && git branch && cd -

# 4. Make a change in the slot to make it dirty
echo "test" >> <SLOT_PATH>/README.md

# 5. Check review status — should show 1 dirty slot
./target/debug/awo review status

# 6. Refresh the slot (re-checks fingerprint and dirty state)
./target/debug/awo slot refresh <SLOT_ID>

# 7. Release the slot (should FAIL because it's dirty)
./target/debug/awo slot release <SLOT_ID>

# 8. Undo the change and release
cd <SLOT_PATH> && git checkout README.md && cd -
./target/debug/awo slot release <SLOT_ID>

# 9. Verify slot is gone
./target/debug/awo slot list

# 10. Repeat with a warm slot to verify retention vs deletion
./target/debug/awo slot acquire <REPO_ID> warm-review --strategy warm
./target/debug/awo slot list
./target/debug/awo slot release <WARM_SLOT_ID>

# 11. Warm release should keep the worktree around for reuse until you delete it
./target/debug/awo slot list
test -d <WARM_SLOT_PATH>

# 12. Explicitly delete the released warm slot/worktree
./target/debug/awo slot delete <WARM_SLOT_ID>
./target/debug/awo slot list

# 13. Recreate a couple of released warm slots, then prune them in one shot
./target/debug/awo slot acquire <REPO_ID> warm-prune-a --strategy warm
./target/debug/awo slot acquire <REPO_ID> warm-prune-b --strategy warm
./target/debug/awo slot release <WARM_PRUNE_A_SLOT_ID>
./target/debug/awo slot release <WARM_PRUNE_B_SLOT_ID>
./target/debug/awo slot prune --repo-id <REPO_ID>
./target/debug/awo slot list
```

---

## Scenario 3: Launch an AI Session on a Slot

**Goal:** Start a real AI session (Claude or shell) in a slot.

```bash
# 1. Acquire a slot
./target/debug/awo slot acquire <REPO_ID> ai-review

# 2. Start a shell session (safest — works without API keys)
./target/debug/awo session start <SLOT_ID> shell "echo 'Hello from Awo Console slot' && ls -la"

# 3. List sessions
./target/debug/awo session list

# 4. Check session log
./target/debug/awo session log <SESSION_ID>
./target/debug/awo session log <SESSION_ID> --stream stderr

# 5. Cancel the session (if still running)
./target/debug/awo session cancel <SESSION_ID>

# 6. Clean up
./target/debug/awo slot release <SLOT_ID>
```

**Bonus — Claude session (requires ANTHROPIC_API_KEY):**
```bash
./target/debug/awo session start <SLOT_ID> claude "List the top 3 files in this repo and briefly describe each" --read-only
```

---

## Scenario 4: Full Team Orchestration — Multi-Agent Mission

**Goal:** Create a team with multiple members, assign tasks, execute them.

```bash
# 1. Init a team
./target/debug/awo team init <REPO_ID> audit-team "Audit code quality and documentation"

# 2. Add two workers
./target/debug/awo team member add audit-team code-reviewer worker \
  --runtime shell --model local-shell --notes "Reviews code for quality issues"

./target/debug/awo team member add audit-team doc-reviewer worker \
  --runtime shell --model local-shell --notes "Reviews documentation completeness"

# 3. Add tasks
./target/debug/awo team plan add audit-team plan-code "Break out code scan" \
  "Convert the broad review idea into one executable task card" \
  --owner-id code-reviewer \
  --deliverable "Executable code review task card" \
  --verification "cargo test"

./target/debug/awo team plan approve audit-team plan-code

./target/debug/awo team plan generate audit-team plan-code code-scan \
  --owner-id code-reviewer \
  --deliverable "Quality report"

./target/debug/awo team task add audit-team doc-scan doc-reviewer \
  "Scan documentation" \
  "Check for missing or outdated docs" \
  --model local-shell \
  --deliverable "Documentation gaps report"

# 4. Show the team manifest
./target/debug/awo team show audit-team

# 5. Start the first task (acquires slot, creates session)
./target/debug/awo team task start audit-team code-scan

# 6. Start the second task
./target/debug/awo team task start audit-team doc-scan

# 7. Check team status
./target/debug/awo team show audit-team

# 8. Check all sessions
./target/debug/awo session list

# 9. Generate a report
./target/debug/awo team report audit-team

# 10. Teardown (cancels sessions, releases slots)
./target/debug/awo team teardown audit-team --force

# 11. Delete the team
./target/debug/awo team delete audit-team
```

---

## Scenario 5: TUI Dashboard — Interactive Control

**Goal:** Use the TUI as the primary control surface for setup and operations.

```bash
# 1. Launch the TUI from a repository you want to register
./target/debug/awo

# 2. Try the setup flows directly in the TUI:
#   a           — open the repo-path form (defaults to the current working directory)
#   Enter       — submit the repo form
#   Tab         — move to the Teams panel
#   c           — create a team for the selected repo
#                 the form now also allows explicit lead runtime/model
#   T           — open Team Dashboard (full-screen team view)
#   m           — add a member in Team Dashboard
#   u           — update the selected member's routing/runtime policy
#   d           — remove the selected non-lead member (with confirmation)
#   p           — add a plan item in Team Dashboard
#   P           — approve the selected draft plan item
#   G           — generate a task card from the selected approved plan item
#   n           — add a task in Team Dashboard
#   D           — delegate the selected task
#   s           — start the selected task in Team Dashboard
#   A           — accept the selected review-ready task card
#   W           — send the selected task card back for rework
#   o           — open the selected task-card log
#   X           — release the selected task-card slot (retain warm / delete fresh)
#   K           — delete the selected released task-card slot
#   [ / ]       — jump directly between review/cleanup task cards in the Team Dashboard

# 3. Explore the operational controls:
#   Tab         — cycle between panels (Repos, Teams, Slots, Sessions)
#   j/k or ↑/↓  — navigate lists
#   /           — filter (type to search, Esc to clear)
#   Enter       — on Slots: start a session prompt; on Sessions: open the log view
#   Tab         — in Team Dashboard: cycle Teams -> Plan -> Members -> Tasks
#   Shift+Tab   — cycle dashboard panes in reverse
#   x           — cancel the selected session
#   X           — release the selected slot
#   r           — refresh review data
#   ?           — help overlay
#   Esc         — back / clear filter
#   q           — quit

# 4. After exploring, clean up any created team/slots
./target/debug/awo team teardown tui-demo --force
./target/debug/awo team delete tui-demo
```

---

## Scenario 5B: Review Closeout — Accept, Rework, Retain, Delete

**Goal:** Close the loop on a completed task card and make an explicit worktree retention decision.

```bash
# 1. Create a repo/team/task and start the task
./target/debug/awo repo add .
./target/debug/awo team init <REPO_ID> closeout-team "Exercise review closeout"
./target/debug/awo team task add closeout-team task-1 lead \
  "Review closeout" \
  "Produce a review-ready result" \
  --model sonnet \
  --deliverable "A reviewed patch"
./target/debug/awo team task start closeout-team task-1 --dry-run

# 2. Open the TUI
./target/debug/awo

# 3. In the Team Dashboard:
#   T           — open Team Dashboard
#   Tab         — focus the Task Cards pane
#   Enter       — open the selected task-card log when a result session exists
#   v           — open the selected task-card diff
#   A           — accept the task card and mark it done
#   C           — cancel the task card without deleting its history
#   S           — supersede the task card with another task card
#   X           — release the slot bound to that task card
#                warm slots are retained for reuse; fresh slots are removed
#   K           — if the slot was warm and released, delete it explicitly

# 4. Re-run the scenario and choose rework instead:
#   W           — send the task card back to todo and clear its review summary
```

```bash
# 5. Exercise immutable recovery from the CLI as well:
./target/debug/awo team task add closeout-team task-2 lead \
  "Replacement task" \
  "Follow-up after supersede" \
  --deliverable "A replacement patch"
./target/debug/awo team task cancel closeout-team task-1
./target/debug/awo team task state closeout-team task-1 todo
./target/debug/awo team task supersede closeout-team task-1 task-2
./target/debug/awo review diff <SLOT_ID>
```

---

## Scenario 6: Daemon Mode — Background Brokerage

**Goal:** Run the daemon explicitly and interact via CLI.

```bash
# 1. Check daemon status
./target/debug/awo daemon status

# 2. The daemon auto-starts on any command, but you can start it explicitly:
#    (Run in a separate terminal)
./target/debug/awod

# 3. In your main terminal, commands now go through the daemon:
./target/debug/awo repo list
./target/debug/awo slot list

# 4. Stop the daemon
./target/debug/awo daemon stop
```

---

## Scenario 7: Runtime Routing — Cost-Aware Selection

**Goal:** Explore how awo decides which runtime to use.

```bash
# 1. List available runtimes and their capabilities
./target/debug/awo runtime list
./target/debug/awo runtime show claude
./target/debug/awo runtime show codex
./target/debug/awo runtime show gemini
./target/debug/awo runtime show shell

# Expect:
# - Claude shows native budget guardrails and structured output support
# - Codex shows native structured output support for exec-mode JSON/JSON-schema flows
# - Gemini shows native structured output support for headless JSON/stream-json output
# - Usage/capacity reporting for provider CLIs is still honest-but-advisory unless the adapter exposes structured telemetry

# 2. Preview routing decisions
./target/debug/awo runtime route-preview --primary claude --primary-model sonnet

# 3. Preview with fallback
./target/debug/awo runtime route-preview \
  --primary claude --primary-model opus \
  --fallback-runtime shell \
  --max-cost-tier standard

# 4. Set runtime pressure (simulates high load)
./target/debug/awo runtime pressure set claude hard_limit
./target/debug/awo runtime pressure list

# 5. See how pressure affects routing
./target/debug/awo runtime route-preview \
  --primary claude --fallback-runtime shell

# 6. Clear pressure
./target/debug/awo runtime pressure clear claude
```

---

## Scenario 8: Task Delegation — Lead/Worker Handoff

**Goal:** Delegate tasks from a lead to workers.

```bash
# 1. Create a team with a lead and workers
./target/debug/awo team init <REPO_ID> delegation-demo "Test task delegation"

./target/debug/awo team member add delegation-demo analyst worker \
  --runtime shell --notes "Runs analysis scripts"

./target/debug/awo team member add delegation-demo fixer worker \
  --runtime shell --notes "Applies fixes"

# 2. Add tasks owned by the lead
./target/debug/awo team task add delegation-demo analyze lead \
  "Analyze codebase" "Find issues in the code" \
  --deliverable "Issue list"

./target/debug/awo team task add delegation-demo fix-issues lead \
  "Fix found issues" "Apply patches for found issues" \
  --deliverable "Patch set" \
  --depends-on analyze

# 3. Delegate tasks to specific workers
./target/debug/awo team task delegate delegation-demo analyze analyst \
  --notes "Focus on error handling patterns" --dry-run

./target/debug/awo team task delegate delegation-demo analyze analyst \
  --notes "Focus on error handling patterns"

# 4. Check the updated manifest
./target/debug/awo team show delegation-demo

# 5. Clean up
./target/debug/awo team teardown delegation-demo --force
./target/debug/awo team delete delegation-demo
```

---

## Scenario 9: Overlap Detection — Parallel Safety

**Goal:** See how awo detects when two slots edit the same files.

```bash
# 1. Acquire two slots on the same repo
./target/debug/awo slot acquire <REPO_ID> feature-a
./target/debug/awo slot acquire <REPO_ID> feature-b

# 2. Make overlapping changes in both slots
echo "change-a" >> <SLOT_A_PATH>/README.md
echo "change-b" >> <SLOT_B_PATH>/README.md

# 3. Check review — should show overlap warning
./target/debug/awo review status

# 4. Clean up
cd <SLOT_A_PATH> && git checkout README.md && cd -
cd <SLOT_B_PATH> && git checkout README.md && cd -
./target/debug/awo slot release <SLOT_A_ID>
./target/debug/awo slot release <SLOT_B_ID>
```

---

## Scenario 10: JSON Mode — Scripting & Automation

**Goal:** Use JSON output for machine-readable results.

```bash
# Every command supports --json
./target/debug/awo repo list --json | python3 -m json.tool

./target/debug/awo slot list --json | python3 -m json.tool

./target/debug/awo team show audit-team --json | python3 -m json.tool

# Useful for scripting:
REPO_ID=$(./target/debug/awo repo list --json 2>/dev/null | python3 -c "
import json, sys
data = json.load(sys.stdin)
if data.get('data'):
    print(data['data'][0]['id'])
")
echo "First repo: $REPO_ID"
```

---

## Scenario 11: Skills Management — Link and Sync

**Goal:** Link repo skills into a runtime directory and keep them in sync.

```bash
# 1. Link skills for a specific runtime (e.g. claude)
./target/debug/awo skills link <REPO_ID> claude --mode symlink

# 2. Check the doctor report — should show linked skills
./target/debug/awo skills doctor <REPO_ID> --runtime claude

# 3. Sync skills (repairs broken links or missing files)
./target/debug/awo skills sync <REPO_ID> claude --mode symlink
```

---

## Scenario 12: Advanced Team Management — Updates and Resets

**Goal:** Update member policies and reset team progress.

```bash
# 1. Create a small team
./target/debug/awo team init <REPO_ID> update-demo "Testing updates"
./target/debug/awo team member add update-demo worker-1 worker --runtime shell

# 2. Update a member's runtime and routing preferences
./target/debug/awo team member update update-demo worker-1 --runtime claude --prefer-local

# 3. Manually override a task's state
./target/debug/awo team task add update-demo manual-task worker-1 "Manual" "Desc" --deliverable "X"
./target/debug/awo team task state update-demo manual-task review

# 4. Reset the entire team (clears task states and slot bindings)
./target/debug/awo team reset update-demo --force

# 5. Check that it returned to 'todo' / 'planning'
./target/debug/awo team show update-demo
```

---

## Scenario 13: Clean Slate — Deleting sessions and Removing Repos

**Goal:** Safely remove repositories and clean up terminal sessions.

```bash
# 1. Register a temporary repo
mkdir /tmp/temp-repo && cd /tmp/temp-repo && git init && touch README.md && git add . && git commit -m "init" && cd -
./target/debug/awo repo add /tmp/temp-repo
REPO_ID=$(./target/debug/awo repo list | grep temp-repo | awk '{print $4}' | tr -d '()')

# 2. Acquire slot and start a dry-run session
./target/debug/awo slot acquire $REPO_ID test-task
SLOT_ID=$(./target/debug/awo slot list --repo-id $REPO_ID | grep active | awk '{print $1}')
./target/debug/awo session start $SLOT_ID shell "ls" --dry-run
SESS_ID=$(./target/debug/awo session list | grep $SLOT_ID | awk '{print $1}')

# 3. Try to remove the repo (should FAIL because of active slot/session)
./target/debug/awo repo remove $REPO_ID

# 4. Cancel the session and release the slot
./target/debug/awo session cancel $SESS_ID
./target/debug/awo slot release $SLOT_ID

# 5. Delete the session record entirely
./target/debug/awo session delete $SESS_ID

# 6. Now remove the repo successfully
./target/debug/awo repo remove $REPO_ID

# 7. Verify it's gone from the list
./target/debug/awo repo list
```

---

## Scenario 14: Repo Clone and Fetch — Remote Repository Lifecycle

**Goal:** Clone a remote repo into AWO's managed directory and refresh it.

```bash
# 1. Clone a small public repo
./target/debug/awo repo clone https://github.com/dtolnay/anyhow.git

# 2. Verify it appeared in the repo list
./target/debug/awo repo list
# Note the REPO_ID (e.g. anyhow-xxxx) and that root is under the configured clone root.
# By default this is inside AWO's data dir, but you can override it with:
#   AWO_CLONES_DIR=/path/to/clones
#   AWO_WORKTREES_DIR=/path/to/worktrees

# 3. Clone with explicit destination
./target/debug/awo repo clone https://github.com/dtolnay/thiserror.git /tmp/thiserror-clone

# 4. Verify both repos registered
./target/debug/awo repo list

# 5. Fetch the first repo (updates remote refs and refreshes metadata)
./target/debug/awo repo fetch <ANYHOW_REPO_ID>

# 6. Inspect context on the cloned repo
./target/debug/awo context pack <ANYHOW_REPO_ID>

# 7. Clean up — remove both repos and delete files
./target/debug/awo repo remove <ANYHOW_REPO_ID>
./target/debug/awo repo remove <THISERROR_REPO_ID>
rm -rf /tmp/thiserror-clone
# Optionally clean the managed clone from your configured clone root.
```

---

## Scenario 15: Warm Slot Strategy — Slot Reuse

**Goal:** Use the warm slot strategy to reuse worktrees across tasks.

```bash
# 1. Acquire a warm slot
./target/debug/awo slot acquire <REPO_ID> warm-task-1 --strategy warm

# 2. Verify it's active with warm strategy
./target/debug/awo slot list
# Note SLOT_ID and SLOT_PATH

# 3. Release the warm slot (it stays on disk, just detaches)
./target/debug/awo slot release <SLOT_ID>

# 4. Verify it's released but still exists on disk
./target/debug/awo slot list
ls <SLOT_PATH>    # should still exist

# 5. Acquire another warm slot — should REUSE the released one
./target/debug/awo slot acquire <REPO_ID> warm-task-2 --strategy warm
# Note: the slot_id should be the same as before, with a new branch name

# 6. Verify reuse — same slot path, new branch
./target/debug/awo slot list
cd <SLOT_PATH> && git branch && cd -

# 7. Release and clean up
./target/debug/awo slot release <SLOT_ID>
```

---

## Scenario 16: Session Modes and Guards — Dry-Run, Read-Only, Timeouts

**Goal:** Exercise session creation options and safety guards.

```bash
# 1. Acquire a slot
./target/debug/awo slot acquire <REPO_ID> session-modes

# 2. Dry-run mode — creates session record but does NOT launch a process
./target/debug/awo session start <SLOT_ID> shell "echo should-not-run" --dry-run
./target/debug/awo session list
# Session should show as Prepared, not Running

# 3. Delete the dry-run session
SESS_ID=<from session list>
./target/debug/awo session delete $SESS_ID

# 4. Make the slot dirty
echo "dirty" >> <SLOT_PATH>/README.md
./target/debug/awo slot refresh <SLOT_ID>

# 5. Try a write session on dirty slot — should FAIL
./target/debug/awo session start <SLOT_ID> shell "echo should-fail"

# 6. Read-only session on dirty slot — should SUCCEED
./target/debug/awo session start <SLOT_ID> shell "cat README.md" --read-only
./target/debug/awo session list

# 7. Verify write-session conflict: start second write session (after cleaning)
cd <SLOT_PATH> && git checkout README.md && cd -
./target/debug/awo slot refresh <SLOT_ID>
./target/debug/awo session start <SLOT_ID> shell "sleep 30" --dry-run
# Try starting another write session on the same slot — should FAIL
./target/debug/awo session start <SLOT_ID> shell "echo conflict"

# 8. Session with timeout
./target/debug/awo session cancel <PENDING_SESS_ID>
./target/debug/awo session start <SLOT_ID> shell "sleep 5 && echo done" --timeout 2
# Session should be killed after 2 seconds

# 9. Clean up
./target/debug/awo slot release <SLOT_ID>
```

---

## Scenario 17: Filtered Listings — Repo-Scoped Queries

**Goal:** Test filtered list commands across slots, sessions, and teams.

```bash
# Setup: register two repos
mkdir /tmp/repo-alpha && cd /tmp/repo-alpha && git init && touch f.txt && git add . && git commit -m "init" && cd -
mkdir /tmp/repo-beta && cd /tmp/repo-beta && git init && touch f.txt && git add . && git commit -m "init" && cd -
./target/debug/awo repo add /tmp/repo-alpha
./target/debug/awo repo add /tmp/repo-beta
./target/debug/awo repo list
# Note ALPHA_ID and BETA_ID

# 1. Acquire slots in both repos
./target/debug/awo slot acquire $ALPHA_ID task-a
./target/debug/awo slot acquire $BETA_ID task-b

# 2. Unfiltered slot list — shows both
./target/debug/awo slot list

# 3. Filtered slot list — only alpha
./target/debug/awo slot list --repo-id $ALPHA_ID

# 4. Filtered slot list — only beta
./target/debug/awo slot list --repo-id $BETA_ID

# 5. Start sessions in both
./target/debug/awo session start <ALPHA_SLOT_ID> shell "echo alpha" --dry-run
./target/debug/awo session start <BETA_SLOT_ID> shell "echo beta" --dry-run

# 6. Filtered session list
./target/debug/awo session list --repo-id $ALPHA_ID
./target/debug/awo session list --repo-id $BETA_ID

# 7. Filtered review status
./target/debug/awo review status --repo-id $ALPHA_ID
./target/debug/awo review status --repo-id $BETA_ID

# 8. Team list with repo filter
./target/debug/awo team init $ALPHA_ID alpha-team "Alpha work"
./target/debug/awo team init $BETA_ID beta-team "Beta work"
./target/debug/awo team list
./target/debug/awo team list --repo-id $ALPHA_ID
./target/debug/awo team list --repo-id $BETA_ID

# 9. Clean up
./target/debug/awo team delete alpha-team
./target/debug/awo team delete beta-team
./target/debug/awo slot release <ALPHA_SLOT_ID>
./target/debug/awo slot release <BETA_SLOT_ID>
./target/debug/awo repo remove $ALPHA_ID
./target/debug/awo repo remove $BETA_ID
rm -rf /tmp/repo-alpha /tmp/repo-beta
```

---

## Scenario 18: Team Member Lifecycle — Show, Remove, Assign Slot

**Goal:** Exercise member operations not covered by basic team scenarios.

```bash
# 1. Create a team with workers
./target/debug/awo team init <REPO_ID> member-demo "Member lifecycle"
./target/debug/awo team member add member-demo alice worker --runtime shell --notes "Backend"
./target/debug/awo team member add member-demo bob worker --runtime shell --notes "Frontend"

# 2. Show individual member details
./target/debug/awo team member show member-demo alice
./target/debug/awo team member show member-demo bob

# 3. Add a task assigned to alice
./target/debug/awo team task add member-demo task-1 alice "Fix bug" "Fix the bug" --deliverable "Patch"

# 4. Try removing alice — should FAIL (has an assigned task)
./target/debug/awo team member remove member-demo alice

# 5. Remove bob — should SUCCEED (no tasks)
./target/debug/awo team member remove member-demo bob
./target/debug/awo team show member-demo
# bob should be gone

# 6. Manually assign a slot to alice
./target/debug/awo slot acquire <REPO_ID> alice-work
./target/debug/awo team member assign-slot member-demo alice <SLOT_ID>
./target/debug/awo team member show member-demo alice
# Should show slot_id bound

# 7. Manually bind slot to task
./target/debug/awo team task bind-slot member-demo task-1 <SLOT_ID>
./target/debug/awo team show member-demo
# task-1 should show the bound slot

# 8. Clean up
./target/debug/awo slot release <SLOT_ID>
./target/debug/awo team delete member-demo
```

---

## Scenario 19: Team Archive — Lifecycle Completion

**Goal:** Take a team through its full lifecycle to archived state.

```bash
# 1. Create and populate a team
./target/debug/awo team init <REPO_ID> archive-demo "Archival test"
./target/debug/awo team member add archive-demo worker-1 worker --runtime shell
./target/debug/awo team task add archive-demo task-1 worker-1 \
  "Run check" "echo done" --deliverable "Output"

# 2. Try archiving right away — should FAIL (task not terminal)
./target/debug/awo team archive archive-demo

# 3. Move task through states to Done
./target/debug/awo team task state archive-demo task-1 in_progress
./target/debug/awo team task state archive-demo task-1 review
./target/debug/awo team task state archive-demo task-1 done
./target/debug/awo team show archive-demo
# task-1 should be done

# 4. Archive the team — should SUCCEED now
./target/debug/awo team archive archive-demo
./target/debug/awo team show archive-demo
# Team status should be Archived

# 5. Delete the archived team
./target/debug/awo team delete archive-demo
```

---

## Scenario 20: Team Teardown Preview — Understanding Before Acting

**Goal:** See what teardown would do before committing.

```bash
# 1. Create a team with active state
./target/debug/awo team init <REPO_ID> teardown-demo "Teardown test"
./target/debug/awo team member add teardown-demo worker-1 worker --runtime shell
./target/debug/awo team task add teardown-demo task-1 worker-1 \
  "Build" "cargo build" --deliverable "Binary"

# 2. Start the task (acquires slot and session)
./target/debug/awo team task start teardown-demo task-1

# 3. Run teardown WITHOUT --force — preview only
./target/debug/awo team teardown teardown-demo
# Should print the teardown plan: slots to release, sessions to cancel, tasks to reset
# Should NOT actually execute anything

# 4. Now force it
./target/debug/awo team teardown teardown-demo --force

# 5. Verify cleanup
./target/debug/awo team show teardown-demo
./target/debug/awo slot list
./target/debug/awo session list

# 6. Clean up
./target/debug/awo team delete teardown-demo
```

---

## Scenario 21: Team Init with Options — Execution Modes and Routing

**Goal:** Explore team initialization options.

```bash
# 1. Init with custom execution mode and routing
./target/debug/awo team init <REPO_ID> routing-team "Routing test" \
  --lead-runtime shell \
  --execution-mode external_slots \
  --fallback-runtime shell \
  --prefer-local \
  --avoid-metered \
  --max-cost-tier standard

# 2. Check the manifest shows the routing config
./target/debug/awo team show routing-team

# 3. Re-init should FAIL (team already exists)
./target/debug/awo team init <REPO_ID> routing-team "Should fail"

# 4. Force re-init overwrites existing team
./target/debug/awo team init <REPO_ID> routing-team "Force re-init" --force

# 5. Verify fresh state
./target/debug/awo team show routing-team

# 6. Clean up
./target/debug/awo team delete routing-team
```

---

## Scenario 22: Task Start Options — Dry-Run and Strategy

**Goal:** Exercise task start with dry-run and warm slot strategy.

```bash
# 1. Set up team
./target/debug/awo team init <REPO_ID> start-demo "Task start options"
./target/debug/awo team member add start-demo worker-1 worker --runtime shell
./target/debug/awo team task add start-demo task-1 worker-1 \
  "Check" "echo hello" --deliverable "Output"

# 2. Dry-run task start — shows what WOULD happen without doing it
./target/debug/awo team task start start-demo task-1 --dry-run
# Should show routing decision, slot plan, session plan — but no actual slot/session created

# 3. Verify nothing was created
./target/debug/awo slot list
./target/debug/awo session list

# 4. Start for real with warm strategy
./target/debug/awo team task start start-demo task-1 --strategy warm

# 5. Verify warm slot was used
./target/debug/awo slot list
# Strategy column should show "warm"

# 6. Clean up
./target/debug/awo team teardown start-demo --force
./target/debug/awo team delete start-demo
```

---

## Scenario 23: Fingerprint Staleness — Detecting Repo Drift

**Goal:** See how AWO detects when a slot's snapshot diverges from the repo.

```bash
# 1. Acquire a slot
./target/debug/awo slot acquire <REPO_ID> stale-test

# 2. Check fingerprint — should be Ready
./target/debug/awo slot refresh <SLOT_ID>
./target/debug/awo slot list
# fingerprint_status should be "ready"

# 3. Make a change in the MAIN repo (not the slot)
echo "new-file" > <REPO_ROOT>/stale-test-marker.txt
cd <REPO_ROOT> && git add stale-test-marker.txt && git commit -m "trigger staleness" && cd -

# 4. Refresh the slot — fingerprint should now be Stale
./target/debug/awo slot refresh <SLOT_ID>
./target/debug/awo slot list
# fingerprint_status should be "stale"

# 5. Review should report stale slot
./target/debug/awo review status

# 6. Try starting a write session — should FAIL on stale slot
./target/debug/awo session start <SLOT_ID> shell "echo should-fail"

# 7. Read-only session on stale slot — should SUCCEED
./target/debug/awo session start <SLOT_ID> shell "cat README.md" --read-only

# 8. Clean up: revert the commit, release slot
cd <REPO_ROOT> && git reset --hard HEAD~1 && cd -
./target/debug/awo slot release <SLOT_ID>
```

**Note:** Replace `<REPO_ROOT>` with the repo's working directory (for example, `/path/to/your/repo`).

---

## Scenario 24: Error Paths — Graceful Failure Handling

**Goal:** Verify the app gives clear errors for invalid operations.

```bash
# 1. Non-existent repo
./target/debug/awo repo remove nonexistent-repo-id
# Expected: unknown repo error

# 2. Non-existent slot
./target/debug/awo slot release nonexistent-slot-id
# Expected: unknown slot error

# 3. Non-existent session
./target/debug/awo session cancel nonexistent-session-id
# Expected: unknown session error

# 4. Non-existent team
./target/debug/awo team show nonexistent-team-id
# Expected: team not found error

# 5. Double-cancel a session
./target/debug/awo slot acquire <REPO_ID> error-test
./target/debug/awo session start <SLOT_ID> shell "echo hi" --dry-run
./target/debug/awo session cancel <SESS_ID>
./target/debug/awo session cancel <SESS_ID>
# Expected: second cancel should fail (already terminal)

# 6. Release an already-released slot
./target/debug/awo slot release <SLOT_ID>
./target/debug/awo slot release <SLOT_ID>
# Expected: second release should fail

# 7. Add duplicate team member
./target/debug/awo team init <REPO_ID> error-team "Error testing"
./target/debug/awo team member add error-team alice worker --runtime shell
./target/debug/awo team member add error-team alice worker --runtime shell
# Expected: duplicate member error

# 8. Invalid runtime name
./target/debug/awo runtime show bogus-runtime
# Expected: unknown runtime error

# 9. Invalid pressure level
./target/debug/awo runtime pressure set claude bogus_level
# Expected: unsupported value error

# 10. Clean up
./target/debug/awo team delete error-team
```

---

## Scenario 25: Debug and Event Bus — Plumbing Inspection

**Goal:** Exercise debug commands and event polling.

```bash
# 1. Debug noop — verifies the full dispatch pipeline
./target/debug/awo debug noop
./target/debug/awo debug noop --label "custom-label"

# 2. Run a few commands to generate events
./target/debug/awo repo list
./target/debug/awo slot list
./target/debug/awo review status

# 3. Poll events via JSON (events are in CommandOutcome data)
#    The event bus is exposed via the events.poll API (available through MCP/RPC).
#    From CLI, events appear inline in non-JSON output.
#    In JSON mode, they appear in the "events" array:
./target/debug/awo repo list --json 2>/dev/null | python3 -c "
import json, sys
data = json.load(sys.stdin)
for event in data.get('events', []):
    print(event)
"

# 4. Debug noop with JSON — shows config paths and event structure
./target/debug/awo debug noop --json 2>/dev/null | python3 -m json.tool
```

---

## Scenario 26: Task State Transitions — Manual Workflow

**Goal:** Walk a task through every possible state transition.

```bash
# 1. Set up team and task
./target/debug/awo team init <REPO_ID> state-demo "State transitions"
./target/debug/awo team member add state-demo worker-1 worker --runtime shell
./target/debug/awo team task add state-demo task-1 worker-1 \
  "State test" "Walk through states" --deliverable "Report"

# 2. Verify initial state is todo
./target/debug/awo team show state-demo

# 3. Transition: todo -> in_progress
./target/debug/awo team task state state-demo task-1 in_progress
./target/debug/awo team show state-demo

# 4. Transition: in_progress -> blocked
./target/debug/awo team task state state-demo task-1 blocked
./target/debug/awo team show state-demo

# 5. Transition: blocked -> in_progress (unblock)
./target/debug/awo team task state state-demo task-1 in_progress
./target/debug/awo team show state-demo

# 6. Transition: in_progress -> review
./target/debug/awo team task state state-demo task-1 review
./target/debug/awo team show state-demo

# 7. Transition: review -> done
./target/debug/awo team task state state-demo task-1 done
./target/debug/awo team show state-demo

# 8. Clean up
./target/debug/awo team delete state-demo
```

---

## Scenario 27: Task Dependencies — Execution Ordering

**Goal:** Verify dependency declarations between tasks.

```bash
# 1. Set up team with dependent tasks
./target/debug/awo team init <REPO_ID> deps-demo "Dependency test"
./target/debug/awo team member add deps-demo worker-1 worker --runtime shell

# 2. Add tasks with dependency chain: build -> test -> deploy
./target/debug/awo team task add deps-demo build worker-1 \
  "Build project" "cargo build" --deliverable "Binary"

./target/debug/awo team task add deps-demo test worker-1 \
  "Run tests" "cargo test" --deliverable "Test report" \
  --depends-on build

./target/debug/awo team task add deps-demo deploy worker-1 \
  "Deploy" "deploy.sh" --deliverable "Deploy receipt" \
  --depends-on test

# 3. Inspect the manifest — verify dependency chain
./target/debug/awo team show deps-demo

# 4. With --json, verify depends_on fields
./target/debug/awo team show deps-demo --json 2>/dev/null | python3 -c "
import json, sys
data = json.load(sys.stdin)
for task in data.get('data', {}).get('tasks', []):
    print(f\"{task['task_id']}: depends_on={task.get('depends_on', [])}\")
"

# 5. Clean up
./target/debug/awo team delete deps-demo
```

---

## Scenario 28: Member Routing Policies — Per-Member Configuration

**Goal:** Configure and verify per-member runtime routing policies.

```bash
# 1. Create team
./target/debug/awo team init <REPO_ID> routing-demo "Routing policies"

# 2. Add members with different routing configs
./target/debug/awo team member add routing-demo fast-worker worker \
  --runtime shell --notes "Fast local worker"

./target/debug/awo team member add routing-demo smart-worker worker \
  --runtime claude --model sonnet \
  --fallback-runtime shell \
  --prefer-local --avoid-metered --max-cost-tier standard \
  --notes "AI worker with cost controls"

# 3. Inspect member routing configurations
./target/debug/awo team member show routing-demo fast-worker
./target/debug/awo team member show routing-demo smart-worker

# 4. Update routing policy for fast-worker
./target/debug/awo team member update routing-demo fast-worker \
  --runtime claude --fallback-runtime shell --no-fallback

# 5. Verify the update
./target/debug/awo team member show routing-demo fast-worker

# 6. Clear fallback with --clear-fallback
./target/debug/awo team member update routing-demo fast-worker --clear-fallback

# 7. Clear all routing with --clear-routing-defaults
./target/debug/awo team member update routing-demo smart-worker --clear-routing-defaults

# 8. Check final state
./target/debug/awo team show routing-demo --json 2>/dev/null | python3 -m json.tool

# 9. Clean up
./target/debug/awo team delete routing-demo
```

---

## Scenario 29: Session Launch Modes — PTY vs Oneshot

**Goal:** Compare PTY-supervised and oneshot session execution.

**Prerequisite:** `tmux` must be installed for PTY mode. (`brew install tmux`)

```bash
# 1. Acquire a slot
./target/debug/awo slot acquire <REPO_ID> launch-test

# 2. Oneshot mode (default if tmux unavailable) — blocking, fire-and-forget
./target/debug/awo session start <SLOT_ID> shell "echo 'oneshot done'" --launch-mode oneshot
./target/debug/awo session list
./target/debug/awo session log <SESSION_ID>

# 3. PTY mode — supervised via tmux (interactive, cancellable)
./target/debug/awo session start <SLOT_ID> shell "echo 'pty done' && sleep 5" --launch-mode pty
./target/debug/awo session list
# Session should show supervisor="tmux"

# 4. Cancel the PTY session while it's running
./target/debug/awo session cancel <PTY_SESSION_ID>
./target/debug/awo session list
# Should show Cancelled status

# 5. Check logs for both
./target/debug/awo session log <ONESHOT_SESSION_ID>
./target/debug/awo session log <PTY_SESSION_ID>
./target/debug/awo session log <PTY_SESSION_ID> --stream stderr

# 6. Clean up
./target/debug/awo slot release <SLOT_ID>
```

---

## Scenario 30: Context Injection — Auto-Context for AI Sessions

**Goal:** Verify that auto-context prepends repo context to AI session prompts.

```bash
# 1. Acquire a slot
./target/debug/awo slot acquire <REPO_ID> context-test

# 2. Dry-run a Claude session — check for context-prepared event
./target/debug/awo session start <SLOT_ID> claude "Describe this repo" --dry-run
./target/debug/awo session list --json 2>/dev/null | python3 -m json.tool
# Events should include SessionContextPrepared with files and packs

# 3. Dry-run a Shell session — Shell does NOT get auto-context
./target/debug/awo session start <SLOT_ID> shell "ls" --dry-run
# Events should NOT include SessionContextPrepared

# 4. Dry-run with --no-auto-context — context explicitly disabled
./target/debug/awo session start <SLOT_ID> claude "Describe this repo" --dry-run --no-auto-context
# Events should NOT include SessionContextPrepared

# 5. Clean up
./target/debug/awo slot release <SLOT_ID>
```

---

## Quick Reference: Command Cheat Sheet

| Action | Command | Scenario |
|--------|---------|----------|
| Register repo | `awo repo add <path>` | 1 |
| Clone repo | `awo repo clone <url> [dest]` | 14 |
| Fetch repo | `awo repo fetch <repo_id>` | 14 |
| Remove repo | `awo repo remove <repo_id>` | 13, 14 |
| List repos | `awo repo list` | 1 |
| Context pack | `awo context pack <repo_id>` | 1 |
| Context doctor | `awo context doctor <repo_id>` | 1 |
| Skills list | `awo skills list <repo_id>` | 1 |
| Skills doctor | `awo skills doctor <repo_id> [--runtime]` | 11 |
| Skills link | `awo skills link <repo_id> <runtime> --mode <m>` | 11 |
| Skills sync | `awo skills sync <repo_id> <runtime> --mode <m>` | 11 |
| Acquire slot | `awo slot acquire <repo_id> <task> [--strategy]` | 2, 15 |
| Release slot | `awo slot release <slot_id>` | 2 |
| Refresh slot | `awo slot refresh <slot_id>` | 2, 23 |
| List slots | `awo slot list [--repo-id]` | 2, 17 |
| Start session | `awo session start <slot_id> <runtime> <prompt> [opts]` | 3, 16, 29 |
| Cancel session | `awo session cancel <session_id>` | 3 |
| Delete session | `awo session delete <session_id>` | 13 |
| Session log | `awo session log <id> [--lines] [--stream]` | 3 |
| List sessions | `awo session list [--repo-id]` | 3, 17 |
| Init team | `awo team init <repo_id> <team_id> <obj> [opts]` | 4, 21 |
| List teams | `awo team list [--repo-id]` | 17 |
| Show team | `awo team show <team_id>` | 4 |
| Add member | `awo team member add <team_id> <id> <role> [opts]` | 4, 28 |
| Show member | `awo team member show <team_id> <member_id>` | 18 |
| Update member | `awo team member update <team_id> <id> [opts]` | 12, 28 |
| Remove member | `awo team member remove <team_id> <member_id>` | 18 |
| Assign slot | `awo team member assign-slot <team_id> <id> <slot>` | 18 |
| Add task | `awo team task add <team_id> <id> <owner> <title> <summary> --deliverable <d>` | 4 |
| Task state | `awo team task state <team_id> <task_id> <state>` | 12, 26 |
| Bind slot | `awo team task bind-slot <team_id> <task_id> <slot>` | 18 |
| Start task | `awo team task start <team_id> <task_id> [opts]` | 4, 22 |
| Delegate task | `awo team task delegate <team_id> <task_id> <member> [opts]` | 8 |
| Reset team | `awo team reset <team_id> [--force]` | 12 |
| Report | `awo team report <team_id>` | 4 |
| Archive team | `awo team archive <team_id> [--force]` | 19 |
| Teardown team | `awo team teardown <team_id> [--force]` | 4, 20 |
| Delete team | `awo team delete <team_id>` | 4 |
| Review status | `awo review status [--repo-id]` | 2, 9, 17 |
| Runtime list | `awo runtime list` | 7 |
| Runtime show | `awo runtime show <runtime>` | 7 |
| Route preview | `awo runtime route-preview --primary <rt> [opts]` | 7 |
| Pressure set | `awo runtime pressure set <runtime> <level>` | 7 |
| Pressure clear | `awo runtime pressure clear <runtime>` | 7 |
| Pressure list | `awo runtime pressure list` | 7 |
| Daemon start | `awo daemon start` | 6 |
| Daemon stop | `awo daemon stop` | 6 |
| Daemon status | `awo daemon status` | 6 |
| Debug noop | `awo debug noop [--label]` | 25 |
| JSON mode | `--json` flag on any command | 10 |
| Launch TUI | `awo` (no subcommand) | 5 |
| Team Dashboard | Press `T` in TUI | 5 |

## TUI Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `?` | Help |
| `Tab` | Next panel |
| `/` | Filter |
| `j`/`k` | Navigate |
| `a` | Add repo |
| `s` | Acquire slot / Start task |
| `Enter` | Start session / View log |
| `x` | Cancel session |
| `X` | Release slot |
| `t` | Start next team task |
| `T` | Team Dashboard |
| `R` | Generate report |
| `r` | Refresh |
| `Esc` | Back |
