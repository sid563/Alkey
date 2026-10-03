#!/usr/bin/env bash
# End-to-end validation of the assessment flow against a running stack.
# Requires: docker compose up -d, cargo run (backend), jq.
set -euo pipefail

API=${API:-http://127.0.0.1:8080}

step() { echo; echo "===== $1 ====="; }

get_token() { # $1=email $2=password
  local login challenge code verify
  login=$(curl -s -X POST "$API/auth/login" -H 'Content-Type: application/json' \
    -d "{\"email\":\"$1\",\"password\":\"$2\"}")
  challenge=$(echo "$login" | jq -r .login_challenge_id)
  code=$(curl -s "$API/dev/email-logs/latest?email=$1" | jq -r .body | grep -oE '[0-9]{6}')
  verify=$(curl -s -X POST "$API/auth/verify-2fa" -H 'Content-Type: application/json' \
    -d "{\"login_challenge_id\":\"$challenge\",\"code\":\"$code\"}")
  echo "$verify" | jq -r .access_token
}

step "1. health"
curl -sf "$API/health" | jq -c .

step "2. seed users"
curl -s -X POST "$API/seed/users" | jq -c '.seeded | map(.email)'

step "3. admin login + 2FA"
ADMIN_TOKEN=$(get_token admin@example.com Admin@123)
[ "$ADMIN_TOKEN" != "null" ] && echo "admin token OK"

step "4. create exactly 5 tasks as admin"
TASK_IDS=()
i=0
for p in high medium low high medium; do
  i=$((i+1))
  id=$(curl -s -X POST "$API/tasks" \
    -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
    -d "{\"title\":\"Task-$i-$p\",\"description\":\"desc $i\",\"status\":\"todo\",\"priority\":\"$p\"}" | jq -r .id)
  TASK_IDS+=("$id")
  echo "created $id ($p)"
done

step "5. assign exactly 3 tasks to James Bond"
curl -s -X POST "$API/tasks/assign" \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d "{\"task_ids\":[\"${TASK_IDS[0]}\",\"${TASK_IDS[1]}\",\"${TASK_IDS[2]}\"],\"assignee_email\":\"jamesbond@example.com\"}" | jq -c .

step "6. james login + 2FA"
JAMES_TOKEN=$(get_token jamesbond@example.com Bond@123)
[ "$JAMES_TOKEN" != "null" ] && echo "james token OK"

step "7. james creates task -> must be 403"
code=$(curl -s -o /tmp/james_create.json -w "%{http_code}" -X POST "$API/tasks" \
  -H "Authorization: Bearer $JAMES_TOKEN" -H 'Content-Type: application/json' \
  -d '{"title":"Nope","description":"Nope","status":"todo","priority":"low"}')
echo "HTTP $code $(cat /tmp/james_create.json)"
[ "$code" = "403" ] || { echo "FAIL: expected 403"; exit 1; }

step "8. james view-my-tasks (expect 3 tasks, cache miss)"
R1=$(curl -s "$API/tasks/view-my-tasks" -H "Authorization: Bearer $JAMES_TOKEN")
echo "$R1" | jq .
[ "$(echo "$R1" | jq .summary.total_assigned_tasks)" = "3" ] || { echo "FAIL: expected 3 tasks"; exit 1; }
[ "$(echo "$R1" | jq .cache.hit)" = "false" ] || { echo "FAIL: expected cache miss"; exit 1; }

step "9. james view-my-tasks again (expect cache hit)"
R2=$(curl -s "$API/tasks/view-my-tasks" -H "Authorization: Bearer $JAMES_TOKEN")
echo "$R2" | jq -c '{total: .summary.total_assigned_tasks, cache: .cache}'
[ "$(echo "$R2" | jq .cache.hit)" = "true" ] || { echo "FAIL: expected cache hit"; exit 1; }

step "10. reassign one of james's tasks away -> his cache must invalidate"
curl -s -X POST "$API/tasks/assign" \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d "{\"task_ids\":[\"${TASK_IDS[0]}\"],\"assignee_email\":\"admin@example.com\"}" | jq -c .
R3=$(curl -s "$API/tasks/view-my-tasks" -H "Authorization: Bearer $JAMES_TOKEN")
echo "$R3" | jq -c '{total: .summary.total_assigned_tasks, cache: .cache}'
[ "$(echo "$R3" | jq .cache.hit)" = "false" ] || { echo "FAIL: expected cache miss after invalidation"; exit 1; }
[ "$(echo "$R3" | jq .summary.total_assigned_tasks)" = "2" ] || { echo "FAIL: expected 2 tasks after reassign"; exit 1; }

step "11. restore: assign task back to james (for frontend demo)"
curl -s -X POST "$API/tasks/assign" \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d "{\"task_ids\":[\"${TASK_IDS[0]}\"],\"assignee_email\":\"jamesbond@example.com\"}" | jq -c .

echo; echo "ALL VALIDATION STEPS PASSED"
