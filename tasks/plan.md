# Implementation Plan: Navigation, Browse + Search, Dashboard Home, Richer Profile

> Targets: `tasks/todo.md` (task list). Related but separate plan: Phase 2
> Notebooks in `.kilo/plans/1791306444022-course-catalog-roles-access.md` —
> untouched here. The former CRLB `tasks/plan.md`/`todo.md` were deleted from
> the working tree before this plan was written (2026-10-12), so these files
> are fresh, not overwritten.

## Overview

Today `/` stacks the full course catalog under a My Courses strip, the topbar
has no real navigation, `/account` shows only a name and role chips, and there
is no search. This plan reorganizes the site into **Home** (personalized
dashboard when signed in; catalog welcome when signed out), **Browse** (all
viewable courses + server-rendered search over course metadata and lesson
titles), and an enriched **Account/profile** panel (identity, roles,
member-since, learning stats, in-progress courses). My Courses cards gain a
progress bar and a Continue link. Everything stays SSR + progressive
enhancement: search is a plain `GET /browse?q=...` form, no client framework,
no new JS beyond optional niceties.

## Architecture Decisions

- **Search = server-side substring match, no index.** ContentStore is fully
  in memory; a `q` filter over course `title`/`description` + lesson `title`
  (case-insensitive contains) is cheap, deterministic, and unit-testable as a
  pure fn in a new `src/web/search.rs`. Full-text lesson bodies rejected for
  v1 (indexing cost + private-content leak surface).
- **Privacy gating is a hard invariant of search and browse**: every hit —
  course card or lesson deep-link — passes `access::can_view` before it is
  rendered. A private course the viewer cannot see must not appear in results
  even by title. Anonymous visitors search public courses only.
- **Home keeps two personas in one route**: signed in → dashboard (My Courses
  with progress + continue, empty state pointing at `/browse`); signed out →
  current hero + catalog (so existing tests and anonymous users keep working).
  `/browse` is the single all-courses page for everyone.
- **Progress on cards reuses the existing `server_progress` fetch** (one PB
  call, cookie-gated) rather than extending `Viewer`; home and account call
  it only when signed in. Per-request PB round-trips stay ≤4 concurrent —
  same localhost-acceptable note as Phase 1.
- **Continue link** = first lesson (course order) not in the completed set;
  if all done, links to the last lesson with a "Review" label. Pure fn, unit
  tested.
- **Member-since** comes from the PB user record `created` field — add
  `created` to `RecordJson`/`AuthUser` parsing (lenient, `#[serde(default)]`).
- **No new widget kinds, no author HTML, no client rendering.** All new markup
  is leptos views stringified server-side (platform invariant).

## Routes after this plan

| Route | Who | Shows |
|---|---|---|
| `/` | signed in | Dashboard: My Courses (progress cards) |
| `/` | signed out / auth off | Hero + public catalog (today's behavior) |
| `/browse` | everyone | All viewable courses + search box (`?q=`), grouped course/lesson hits |
| `/account` | signed in | Profile: identity, roles, member-since, stats, in-progress list, sign out |
| `/account` | signed out | Sign-in panel (unchanged) |
| `/admin/*` | admins | Unchanged |

Topbar nav: **Brillante** (brand → `/`) · **Home** · **Browse** · search icon
link to `/browse` · **Account** · **Admin** (admin only).

## Task List

### Phase 1: Navigation & Browse

- [x] Task 1: Add `/browse` page + restructure topbar nav (Home/Browse)
- [x] Task 2: Server-rendered search on `/browse` (courses + lesson titles)

### Checkpoint: Navigation
- [x] Tests pass, builds clean; browse + search respect course privacy

### Phase 2: Dashboard & Profile

- [x] Task 3: My Courses progress cards (bar + continue) and personalized home
- [x] Task 4: Enriched `/account` profile (identity, member-since, stats, in-progress)

### Checkpoint: Core Features
- [x] End-to-end signed-in flow works: browse → subscribe → home dashboard → continue lesson → profile stats

### Phase 3: Verification & Docs

- [x] Task 5: Live-stack tests (pocketbase_live) for dashboard/profile assertions
- [x] Task 6: Docs — AGENTS.md + README (routes, nav, search privacy policy)

### Checkpoint: Complete
- [x] All acceptance criteria met; playwright sign-off; ready for review

## Task details

See `tasks/todo.md` for the checklist; full acceptance criteria and
verification per task live there.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Search leaks a private course's title/lesson names to unauthorized viewers | High | Single pure search fn filters through `can_view` before returning hits; dedicated unit + integration tests with a private fixture course |
| Home behavior change breaks existing integration tests / anonymous UX | Med | Signed-out home keeps today's hero+catalog markup exactly; `home_lists_registered_courses` must keep passing untouched |
| Extra PB round-trip per home/account render for progress | Low | Cookie-gated, concurrent, degrades to no-progress rendering (existing `server_progress` policy) |
| `AuthUser` gains a field (created) and every constructor/test literal must change | Low | Add with `#[serde(default)]` + `Default` derive already present; struct-update syntax in tests |
| Course with zero lessons → divide-by-zero / empty continue href | Low | Pure fn handles 0/total; continue falls back to outline link |

## Open Questions

- None blocking. (Full-text lesson-body search and a dedicated `/profile`
  page were consciously deferred.)
