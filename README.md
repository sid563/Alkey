# Task Management App — Rust API + React UI

Full-stack task management app with email-password login, email-based 2FA,
JWT auth, role-based access (admin/staff), task assignment, and per-user
Redis caching.

## Tech stack

| Area | Technology |
|---|---|
| Language | Rust stable, edition 2021 |
| Web framework | Actix Web 4 (async, Tokio runtime) |
| Database | PostgreSQL 16 — **local via Docker**, no cloud dependency |
| DB access | SQLx 0.8 (compile-checked async queries, connection pool) |
| Migrations | SQLx migrations in `backend/migrations/`, auto-run on startup |
| Cache | Redis 7 — **local via Docker**, per-user cache + invalidation |
| Infrastructure | Docker Compose (Postgres + Redis with healthchecks and a persistent volume) |
| Password hashing | Argon2id (`argon2` crate) — also used for hashing 2FA codes |
| Auth tokens | JWT via `jsonwebtoken` (HS256, 24h expiry) |
| Email (2FA codes) | `email_logs` table + console (dev inbox), **plus real delivery via Google SMTP** (`lettre`, personal Gmail account with an App Password, fire-and-forget) |
| Serialization | Serde / serde_json for all request & response DTOs |
| Config | dotenvy (`.env`), all settings via environment variables |
| Logging | tracing + tracing-subscriber |
| IDs / time | UUID v4 (`uuid`), UTC timestamps (`chrono`) |
| Backend testing | cargo test (unit) + reqwest-based e2e integration test (`--ignored`) |
| Frontend | React 19 + Vite, plain CSS (no UI framework), separated API service layer |
| Lint / format | clippy (zero warnings) + rustfmt |

```
backend/   Rust API (src/auth, src/tasks, migrations/, tests/e2e.rs)
ui/        React app (src/api service layer, src/pages, src/components)
scripts/   validate.sh — scripted end-to-end validation flow
```

## Setup

Prerequisites: Rust stable, Node 18+, Docker.

```bash
# 1. Infrastructure (PostgreSQL + Redis)
docker compose up -d

# 2. Backend — migrations run automatically on startup
cd backend
cp .env.example .env        # defaults match docker-compose; set a real JWT_SECRET
cargo run                   # serves http://127.0.0.1:8080

# 3. Frontend
cd ui
cp .env.example .env        # points at http://127.0.0.1:8080
npm install
npm run dev                 # serves http://localhost:5173
```

### Seed users

`POST /seed/users` (idempotent) creates:

| User | Email | Password | Role |
|---|---|---|---|
| Admin | admin@example.com | Admin@123 | admin |
| James Bond | jamesbond@example.com | Bond@123 | staff |

```bash
curl -s -X POST http://127.0.0.1:8080/seed/users | jq
```

## API

| Endpoint | Auth | Purpose |
|---|---|---|
| `GET /health` | — | Liveness incl. DB + Redis connectivity |
| `POST /seed/users` | — | Seed Admin + James Bond (idempotent) |
| `POST /auth/login` | — | Verify password, create 2FA challenge, "send" code. Returns `login_challenge_id`, never a JWT |
| `GET /dev/email-logs/latest?email=` | — | Dev-only inbox: latest email for an address |
| `POST /auth/verify-2fa` | — | Verify one-time code, returns JWT |
| `POST /tasks` | admin | Create a task |
| `GET /tasks` | admin | List all tasks (used by the admin screen to pick tasks to assign) |
| `POST /tasks/assign` | admin | Assign tasks to a user by email (all-or-nothing) |
| `GET /tasks/view-my-tasks` | any | Logged-in user's tasks, with cache metadata |

## Validation flow

Scripted (requires `jq`): `./scripts/validate.sh` runs the entire flow below and
asserts every expectation, including cache invalidation on reassignment.

Manual, via curl:

```bash
API=http://127.0.0.1:8080

# 1. Seed users
curl -s -X POST $API/seed/users | jq

# 2. Login as Admin -> returns login_challenge_id (no JWT yet)
curl -s -X POST $API/auth/login -H 'Content-Type: application/json' \
  -d '{"email":"admin@example.com","password":"Admin@123"}' | jq

# 3. Get the 2FA code from the dev email log (also printed to API console)
curl -s "$API/dev/email-logs/latest?email=admin@example.com" | jq -r .body

# 4. Verify 2FA -> returns the Admin JWT
curl -s -X POST $API/auth/verify-2fa -H 'Content-Type: application/json' \
  -d '{"login_challenge_id":"<CHALLENGE_ID>","code":"<CODE>"}' | jq
# export ADMIN_TOKEN=<access_token>

# 5. Create exactly 5 tasks
for p in high medium low high medium; do
  curl -s -X POST $API/tasks -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H 'Content-Type: application/json' \
    -d "{\"title\":\"Task-$p\",\"description\":\"desc\",\"status\":\"todo\",\"priority\":\"$p\"}" | jq .id
done

# 6. Assign exactly 3 of them to James Bond
curl -s -X POST $API/tasks/assign -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"task_ids":["<ID1>","<ID2>","<ID3>"],"assignee_email":"jamesbond@example.com"}' | jq

# 7-8. Login + 2FA as James Bond (same as steps 2-4) -> JAMES_TOKEN

# 9. James tries to create a task -> 403 Forbidden
curl -i -X POST $API/tasks -H "Authorization: Bearer $JAMES_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"title":"Nope","description":"","status":"todo","priority":"low"}'

# 10-11. View my tasks twice: first cache.hit=false, then cache.hit=true
curl -s $API/tasks/view-my-tasks -H "Authorization: Bearer $JAMES_TOKEN" | jq .cache
curl -s $API/tasks/view-my-tasks -H "Authorization: Bearer $JAMES_TOKEN" | jq .cache
```

Or do the same through the UI at http://localhost:5173 — the login screen has a
"Fetch code from dev inbox" helper so you never leave the browser.

## Final Validation Response

Actual output of `GET /tasks/view-my-tasks` as James Bond (first call after
assignment — cache miss):

```json
{
  "cache": {
    "hit": false
  },
  "summary": {
    "total_assigned_tasks": 3
  },
  "tasks": [
    {
      "assigned_to": "jamesbond@example.com",
      "description": "desc 1",
      "id": "04ea34b7-5cd5-4fa6-acff-25354fb629d7",
      "priority": "high",
      "status": "todo",
      "title": "Task-1-high"
    },
    {
      "assigned_to": "jamesbond@example.com",
      "description": "desc 2",
      "id": "8514509a-6230-47a5-b0df-90de42a7fe38",
      "priority": "medium",
      "status": "todo",
      "title": "Task-2-medium"
    },
    {
      "assigned_to": "jamesbond@example.com",
      "description": "desc 3",
      "id": "2770b69e-dddc-40e6-9604-18692406d4bc",
      "priority": "low",
      "status": "todo",
      "title": "Task-3-low"
    }
  ],
  "user": {
    "email": "jamesbond@example.com",
    "role": "staff"
  }
}
```

The second identical call returns `"cache": { "hit": true }` with the same tasks.

## Design notes

**2FA.** `POST /auth/login` verifies the Argon2id password hash, generates a
6-digit code, stores only its Argon2 hash in `login_challenges` (5-minute
expiry, single-use), and writes the plaintext code to the `email_logs` table +
console — the local stand-in for email delivery. Verification runs in a
transaction with `SELECT ... FOR UPDATE`, so a code can never be redeemed twice
even by concurrent requests; a wrong code is rejected without consuming the
challenge. A JWT (claims: `sub`, `email`, `role`, 24h expiry) is issued only
after successful verification.

Optional real delivery: set the `SMTP_*` variables in `backend/.env` (Gmail App
Password works) and codes are also sent via SMTP, fire-and-forget — login never
depends on the mail server. `SMTP_DEV_REDIRECT_TO` routes all mail to one real
inbox since seeded users have fake addresses.

**Roles.** The JWT carries the role, an `AuthedUser` extractor validates the
token (401 on missing/invalid), and admin-only handlers return 403 with a JSON
error body for staff tokens.

**Caching.** Cache-aside with per-user keys (`v1:tasks:{user_id}`),
JSON-serialized task list, 60s TTL. Writes never update the cache:
`POST /tasks/assign` runs one transaction (validating every task id and the
assignee before writing anything), then deletes the cache keys of the new
assignee *and all previous assignees* after commit. Explicit invalidation
provides freshness; the TTL is a self-healing backstop for missed or failed
invalidations. Redis failures degrade to direct DB reads — the cache is an
optimization, never a dependency. Only the task list is cached (not the
response envelope), so `cache.hit` always reflects the current request.

## Tests

```bash
cd backend
cargo test                  # unit tests: JWT round-trip/expiry, 2FA hash/verify, code format
cargo test -- --ignored     # full e2e flow (requires docker compose up + cargo run)
```

The e2e test covers: seed, challenge-not-token login, wrong/reused code
rejection, 5 tasks created, 3 assigned, staff 403, cache miss→hit, cache
invalidation on reassignment, and unauthenticated 401.

## Environment variables

See `backend/.env.example` (HOST, PORT, DATABASE_URL, REDIS_URL, JWT_SECRET,
JWT_TTL_HOURS, TWOFA_TTL_MINUTES, CACHE_TTL_SECONDS, CORS_ORIGIN, optional
SMTP_*) and `ui/.env.example` (VITE_API_URL).

## Screenshots

See [screenshots/](screenshots/): login screen, admin task creation/assignment,
James Bond's 3 assigned tasks with the cache badge, and the 403 message when
James tries to create a task.
