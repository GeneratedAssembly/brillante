# Task List: Navigation, Browse + Search, Dashboard Home, Richer Profile

Plan: `tasks/plan.md`. Repo verification commands: `cargo test`,
`cargo clippy`, `cargo run` (server at http://127.0.0.1:3000, PB via
`./pocketbase/run.sh`). UI sign-off via `playwright-cli`.

## Task 1: Add `/browse` page + restructure topbar nav

**Description:** Introduce a dedicated all-courses page `GET /browse` showing
every course the viewer can see (same `can_view` filtering and card markup as
today's catalog section), and restructure the `Shell` topbar into real
navigation: brand → `/`, **Home** → `/`, **Browse** → `/browse`, Account,
Admin. `/` itself keeps its current content in this task (dashboard change is
Task 3); only the nav and the new route land.

**Acceptance criteria:**
- [x] `GET /browse` renders a `course-grid` of viewable courses; subscribe toggle present when signed in with auth enabled; Private chip preserved
- [x] Topbar contains Home and Browse links on every page (incl. 404 and admin pages)
- [x] Private courses a viewer cannot see are absent from `/browse` (anonymous and signed-in-without-access)

**Verification:**
- [x] `cargo test` passes; new fixture tests in `tests/web.rs`: browse lists fixture courses; nav links present; private fixture course hidden from anonymous browse
- [x] `cargo clippy` clean; `cargo build` succeeds
- [x] Manual: `curl -s localhost:3000/browse | grep -c course-card` > 0 with server running

**Dependencies:** None
**Files likely touched:**
- `src/web/pages.rs` (browse handler, Shell nav)
- `src/web/mod.rs` (route)
- `src/web/site.css` (topnav tweaks)
- `tests/web.rs`

**Estimated scope:** Medium (3-4 files)

## Task 2: Server-rendered search on `/browse`

**Description:** Add a pure search module (`src/web/search.rs`): given the
store, query, viewer and access map, return grouped hits — courses whose
title/description contain `q` (case-insensitive), plus lessons whose title
contains `q` rendered as sub-links under their course. Wire `GET /browse?q=`
to a plain `<form method="get">` search box; render "N results" or an empty
state. No JS required.

**Acceptance criteria:**
- [x] `search(store, q, viewer, map)` is a pure fn with colocated unit tests: case-insensitivity, course + lesson hits, empty query → no search performed, **private-course hits suppressed** for viewers without access
- [x] `/browse?q=…` renders matched courses with lesson deep-links (`/course/<c>/<l>`); non-matching cards hidden
- [x] The search box retains the query after submit; empty-result state links back to unfiltered `/browse`

**Verification:**
- [x] `cargo test` (unit + `tests/web.rs` fixture: lesson-title hit deep-links correctly; private fixture course never appears in results anonymously)
- [x] `cargo clippy` clean
- [x] Manual: `curl -s 'localhost:3000/browse?q=loops'` shows the fixture lesson link

**Dependencies:** Task 1
**Files likely touched:**
- `src/web/search.rs` (new)
- `src/web/pages.rs`
- `src/web/mod.rs` (module registration)
- `src/web/site.css`
- `tests/web.rs`

**Estimated scope:** Medium (4-5 files)

## Checkpoint: Navigation (after Tasks 1-2)

- [x] `cargo test` + `cargo clippy` clean
- [x] Browse + search respect course privacy in fixture tests
- [x] Review with human before Phase 2

## Task 3: My Courses progress cards + personalized home

**Description:** Extract a shared `my_courses` section: cards show a progress
bar (completed/total lessons) and a **Continue** button linking to the first
unfinished lesson (or **Review** → last lesson when complete) — computed by a
pure fn with unit tests. Home becomes persona-aware: signed in → dashboard
(My Courses section; empty state points to `/browse`; hero shrinks), signed
out → today's hero + catalog unchanged. Home fetches server progress only
when signed in.

**Acceptance criteria:**
- [x] `continue_target(course, completed_set) -> (href, label)` pure fn, unit tested (first unfinished, all-done → review, empty course → outline)
- [x] Signed-in home renders My Courses cards with `x/y` progress and a Continue link; unsubscribed courses are NOT on the dashboard
- [x] Signed-out (and auth-disabled) home markup is byte-compatible with today's tests (`home_lists_registered_courses` still passes unmodified)
- [x] PB unreachable → dashboard renders cards without progress (degrade, never error)

**Verification:**
- [x] `cargo test`: fixture asserts progress bar + continue href for a signed-in viewer with seeded progress rows (extend the `fixture_with_auth` pattern); existing home tests untouched and green
- [x] `cargo clippy` clean
- [x] Manual: sign in via dev PB, complete a lesson, reload `/`, confirm bar + Continue target

**Dependencies:** Task 1 (nav context; browse link target for empty state)
**Files likely touched:**
- `src/web/pages.rs` (home, shared section, card component)
- `src/web/site.css` (progress bar styles)
- `tests/web.rs`

**Estimated scope:** Medium (3 files)

## Task 4: Enriched `/account` profile panel

**Description:** Replace the sparse signed-in panel with a profile: display
name + email, role chips (exists), **member since** (PB record `created`,
parsed leniently into `AuthUser`), stat row (courses subscribed · lessons
completed · courses in progress), and an in-progress list reusing Task 3's
continue cards. Sign-out form unchanged. Signed-out panel unchanged.

**Acceptance criteria:**
- [x] `RecordJson`/`AuthUser` parse `created` leniently (`#[serde(default)]`, unit test incl. missing field)
- [x] Signed-in `/account` shows email, member-since date, and the three stats computed from subscriptions + progress (not from course-page data)
- [x] In-progress list shows only subscribed courses with 0 < completed < total, each with a Continue link
- [x] Auth-disabled behavior untouched (`account_page_reports_disabled_without_pocketbase` passes)

**Verification:**
- [x] `cargo test` (unit for parse; `tests/web.rs` fixture for panel markup when signed in) + `POCKETBASE_TEST_URL=… cargo test --test pocketbase_live` sanity for created-field round trip
- [x] `cargo clippy` clean
- [x] Manual: signed-in `/account` in browser shows stats matching actual progress

**Dependencies:** Task 3 (shared continue/progress component)
**Files likely touched:**
- `src/pb.rs` (created field + tests)
- `src/web/auth.rs` (account page)
- `src/web/pages.rs` (exposed shared section)
- `src/web/site.css`
- `tests/web.rs`

**Estimated scope:** Medium (4-5 files)

## Checkpoint: Core Features (after Tasks 3-4)

- [x] End-to-end signed-in flow: browse → search → subscribe → home dashboard → continue lesson → account stats — verified against dev PB
- [x] `cargo test` + `cargo clippy` clean
- [x] Review with human before Phase 3

## Task 5: Live-stack tests for dashboard/profile

**Description:** Extend `tests/pocketbase_live.rs` to cover the new
signed-in surfaces against a real PB: after signup + subscribe + progress
mark, `GET /` contains the continue href and progress count; `GET /account`
contains member-since and stat numbers; `GET /browse?q=` for a
role-gated private course is empty for the plain user and present after
grant.

**Acceptance criteria:**
- [x] New live tests pass with `POCKETBASE_TEST_URL` set and skip cleanly without it (existing pattern)
- [x] Tests create their own throwaway users/courses via the fixture, never touch shipped content

**Verification:**
- [x] `POCKETBASE_TEST_URL=http://127.0.0.1:8090 cargo test --test pocketbase_live` green
      (note: private-course search gating is covered by `search.rs` unit tests;
      a live variant needs admin credentials in the test env — deferred)
- [x] `cargo test` green (live suite skipped)

**Dependencies:** Tasks 3, 4
**Files likely touched:**
- `tests/pocketbase_live.rs`

**Estimated scope:** Small (1 file)

## Task 6: Docs — AGENTS.md + README

**Description:** Document the new routes (`/browse`, `?q=`), nav structure,
dashboard/profile behavior, and the search privacy policy ("search results
are gated by `can_view`; private courses never appear as hits") in AGENTS.md
(repo map + gotchas) and README (commands/screens note).

**Acceptance criteria:**
- [x] AGENTS.md route table / repo map reflects `src/web/search.rs` and `/browse`
- [x] README describes home-persona behavior and search for readers

**Verification:**
- [x] Docs match the shipped markup (spot-check classes/hrefs against pages.rs)

**Dependencies:** Tasks 1-4
**Files likely touched:**
- `AGENTS.md`
- `README.md`

**Estimated scope:** Small (2 files)

## Checkpoint: Complete

- [x] All task acceptance criteria met
- [x] `cargo test`, `cargo clippy` clean; live suite green against dev PB
- [x] Playwright sign-off: nav on every page; signed-out `/` unchanged; `/browse` + search results; signed-in dashboard with progress; `/account` stats
- [x] Ready for human review

## Final UI sign-off (playwright-cli, after Task 4)

- [x] `open http://127.0.0.1:3000/` signed out → hero + catalog, nav shows Home/Browse/Account
- [x] `/browse?q=<fixture term>` → matched cards + lesson deep-links
- [x] Sign in (dev account) → `/` dashboard shows subscribed course with bar + Continue; `/account` shows email, member-since, stats
- [x] `cookie-list` shows only `brillante.session` (httpOnly) — no new cookie surface
