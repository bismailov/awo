#!/bin/bash
set -e

echo "--- Awo Smoke Test ---"

# 1. Setup clean environment
export AWO_DATA_DIR="/tmp/awo-smoke-data"
export AWO_CONFIG_DIR="/tmp/awo-smoke-config"
rm -rf "$AWO_DATA_DIR" "$AWO_CONFIG_DIR"
mkdir -p "$AWO_DATA_DIR" "$AWO_CONFIG_DIR"
echo "Using data dir: $AWO_DATA_DIR"

# Ensure no stale daemons
pkill awod || true
sleep 1

# 2. Add current repo
./target/debug/awo repo add . --json > /dev/null
echo "✓ Repo added (and daemon auto-started)"

# 3. Get repo ID
REPO_ID=$(./target/debug/awo repo list --json | python3 -c "import sys, json; data = json.load(sys.stdin); print(data['data'][0]['id'] if data['data'] else 'none')")
if [ "$REPO_ID" == "none" ]; then
    echo "✗ Failed to get Repo ID"
    exit 1
fi
echo "✓ Repo ID: $REPO_ID"

# 4. Init team
# Usage: awo team init <REPO_ID> <TEAM_ID> <OBJECTIVE>
./target/debug/awo team init "$REPO_ID" smoke-team "Smoke test objective" --json > /dev/null
echo "✓ Team init"

echo "Listing config dir after init:"
ls -R "$AWO_CONFIG_DIR"

# 5. Add member
./target/debug/awo team member add smoke-team worker1 worker --runtime shell --json > /dev/null
echo "✓ Member added"

# 6. Add task
SMOKE_FILE="$AWO_DATA_DIR/smoke_success.txt"
./target/debug/awo team task add smoke-team task1 worker1 "Task 1" "echo 'success' > $SMOKE_FILE" --deliverable "smoke_success.txt" --json > /dev/null
echo "✓ Task added"

# 7. Start task
echo "Starting task..."
./target/debug/awo team task start smoke-team task1 --json > /dev/null
echo "✓ Task started"

# 8. Wait for completion
for i in {1..10}; do
    RAW_JSON=$(./target/debug/awo team show smoke-team --json)
    STATUS=$(echo "$RAW_JSON" | python3 -c "import sys, json; 
try:
    data = json.load(sys.stdin)
    if data['data'] is None:
        print(f'ERROR_DATA_NULL: {data.get(\"error\")}')
    else:
        tasks = data['data'].get('tasks', [])
        print(next((t['state'] for t in tasks if t['task_id'] == 'task1'), 'NOT_FOUND'))
except Exception as e:
    print(f'PYTHON_ERROR: {e}')
")
    
    if [ "$STATUS" == "review" ] || [ "$STATUS" == "done" ]; then
        echo "✓ Task status: $STATUS"
        break
    fi
    
    if [[ "$STATUS" == "PYTHON_ERROR"* ]] || [[ "$STATUS" == "ERROR_DATA_NULL"* ]]; then
        echo "✗ Error in team show output: $STATUS"
        ./target/debug/awo daemon stop --json > /dev/null || true
        exit 1
    fi

    echo "...waiting (status: $STATUS)"
    sleep 1
done

# 9. Verify output
if [ -f "$SMOKE_FILE" ]; then
    CONTENT=$(cat "$SMOKE_FILE")
    if [ "$CONTENT" == "success" ]; then
        echo "✓ Output verified: $CONTENT"
    else
        echo "✗ Output mismatch: $CONTENT"
        ./target/debug/awo daemon stop --json > /dev/null || true
        exit 1
    fi
else
    echo "✗ Output file missing: $SMOKE_FILE"
    ./target/debug/awo daemon stop --json > /dev/null || true
    exit 1
fi

# 10. Stop daemon
./target/debug/awo daemon stop --json > /dev/null
echo "✓ Daemon stopped"

echo "--- SMOKE TEST PASSED ---"
rm -rf "$AWO_DATA_DIR" "$AWO_CONFIG_DIR"
