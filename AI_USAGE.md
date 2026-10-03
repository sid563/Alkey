# AI Usage Disclosure

## Tools used

- **Claude Code** (Anthropic, model: Claude Fable 5) was used throughout this
  assignment, driven interactively from the IDE.

## Tech stack (full list)

Rust (edition 2021) · Actix Web 4 · Tokio · SQLx 0.8 · **PostgreSQL 16 (local,
Docker)** · **Redis 7 (local, Docker)** · Docker Compose · SQLx migrations ·
Argon2id · JWT (`jsonwebtoken`) · Serde · **Google SMTP via `lettre` (personal
Gmail + App Password) for real 2FA email delivery**, with an `email_logs`
table + console as the always-on dev inbox · dotenvy · tracing · UUID/chrono ·
React 19 + Vite frontend (plain CSS, separated API layer) · clippy + rustfmt ·
cargo unit tests + reqwest e2e integration test.

## Strategy discussions before development

No code was written until the design was settled. The pre-development sessions
covered, in order:

**1. Reading the assessment as a grader would.** We first broke the brief into
what is actually being evaluated: (a) the exact validation flow must pass
verbatim — so response shapes like `summary.total_assigned_tasks`,
`cache.hit`, and `assigned_to` as an email are contracts, not suggestions;
(b) the security details a reviewer greps for — hashed one-time 2FA codes,
expiry, single use, Argon2, 403 on staff; (c) honest transparency artifacts.
The plan was designed backwards from the grader's curl script.

**2. Database choice.** An earlier draft of the plan targeted MongoDB. We
discussed the trade-offs and switched to PostgreSQL (which the final brief
prefers): real multi-row transactions replace Mongo atomicity workarounds,
`SELECT ... FOR UPDATE` gives race-free one-time code redemption and
assignment batches, and SQLx migrations keep setup at two commands. SQLx was
chosen over Diesel/SeaORM for being async-native alongside Actix.

**3. 2FA design.** Decided before implementation: login returns a
`login_challenge_id` and never a JWT; the 6-digit code is stored only as an
Argon2 hash in `login_challenges` while the plaintext goes to `email_logs`
(the dev stand-in for email) and the console; verification runs in a
transaction with a row lock so a code can never be redeemed twice even by
concurrent requests, and a wrong code is rejected *without* consuming the
challenge; the JWT carries `sub`/`email`/`role` so role checks don't hit the
database per request.

**4. Caching strategy — a dedicated design discussion.** This was the deepest
pre-development conversation, and the implementation follows it exactly:

- **Cache-aside, not write-through**: writes invalidate, reads populate.
  Rebuilding a user's task list inside the assign path would couple write
  latency to read-side shape and double the bugs; deleting a key is O(1) and
  cannot be wrong.
- **Versioned per-user keys** (`v1:tasks:{user_id}`), keyed by user id (it's
  in the JWT; emails can change).
- **Cache the task list only, not the response envelope** — otherwise the
  second request would replay `"hit": false` from Redis. `cache.hit` is
  metadata about *this* request.
- **Invalidation is the consistency mechanism; the 60s TTL is the backstop**
  for missed or failed invalidations, crashes between commit and DEL, or
  future endpoints that forget to invalidate. Every bug self-heals in a
  minute.
- **Invalidate previous assignees, not just the new one** — when a task moves
  from James to Admin, James's cached "3 tasks" is stale too. The assign
  handler captures previous `assigned_to_id`s under `FOR UPDATE` before
  updating.
- **Invalidate after commit, not before**, so a concurrent read can't
  repopulate the cache from pre-commit data and silently undo the
  invalidation. We explicitly discussed the remaining tiny post-commit read
  race, decided the bulletproof fixes (versioned CAS keys, tombstones) are
  not worth it at this scale, and accepted it as TTL-covered — documented in
  the README rather than pretended away.
- **Redis failures degrade to DB reads** — the cache is an optimization,
  never a dependency. A failed post-commit DEL is logged, not propagated;
  failing an assignment because cache cleanup failed would invert priorities.
- Consciously skipped and documented: stampede protection (per-user keys make
  it irrelevant), `UNLINK` vs `DEL` (tiny values), per-field hashes
  (contradicts invalidate-don't-update).

**5. Execution plan.** Work was split into five steps, each with a "done
when" gate verified against the running stack before moving on: scaffold +
infrastructure → auth vertical → tasks + caching → frontend → tests + docs.
We also planned verification beyond the brief's script: step 10 of
`scripts/validate.sh` reassigns a task away from James and asserts a cache
miss with the reduced count — proving the invalidation rule the brief states
but its script never actually tests.

## How it was used during development

Claude generated the backend and frontend code following the agreed design,
validating each step with curl against the live stack. The UI was verified
end-to-end with a headless browser run that produced the screenshots in
`screenshots/`. Real SMTP delivery (Gmail App Password, `lettre`,
fire-and-forget so login never depends on the mail server) was added on top;
credentials live only in the gitignored `.env`.

## Manual decisions and changes

- Choice of stack and the decision to switch MongoDB → PostgreSQL.
- Direction of the caching design discussion and sign-off on its trade-offs.
- Adding real SMTP delivery with my own Gmail App Password, and fixing the
  account/credential mismatch in `.env` during testing.
- Review of the generated code and the validation evidence at each step.

## Known AI-related caveats (bugs caught by actually testing)

- The e2e test originally extracted "the first six digits" from the 2FA email
  and picked up the digit in the word "2FA". Caught because the integration
  test runs against the live stack; fixed to match a run of exactly six
  digits.
- The mailer initially treated blank `.env.example` values as configured,
  producing an empty recipient; fixed to treat empty env values as unset.

I understand the submitted code and can walk through any part of it: the 2FA
transaction, the JWT extractor, the cache invalidation path, and the frontend
auth state handling.
