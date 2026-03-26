#!/bin/bash
# Helper: release any slot with "manual" in the branch name
awo slot list --json 2>/dev/null | python3 -c "
import json, sys
slots = json.load(sys.stdin).get('data', [])
for s in slots:
    if 'manual' in s.get('branch_name', ''):
        print(s['id'])
" | while read -r slot_id; do
    awo slot release "$slot_id"
done
