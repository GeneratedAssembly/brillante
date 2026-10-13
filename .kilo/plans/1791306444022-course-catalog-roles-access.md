# Plan: Course Catalog, Subscriptions, Roles, Course Privacy & Notebooks

> **Status:** Phase 1 (catalog, subscriptions, roles, access, OAuth, admin UI)
> and **Phase 2 (Notebooks) are complete** (2026-10-12) — see "Phase 2 status"
> below for implementation deviations. The UI reorganization
> (browse/search/dashboard/profile) is tracked separately in `tasks/plan.md` +
> `tasks/todo.md` (also complete).
>
> **Active debug task (below):** SSO buttons (Continue with Google/GitHub) are
> not appearing — ordered diagnostic plan in section "Debug task: missing SSO
> buttons" at the end of this file. **RESOLVED 2026-10-12** — see "Resolution"
> at the end of that section (root cause A: PB container down + image pruned;
> plus a latent PB 0.23+ schema bug that broke all auth decoding, fixed by
> migration `1791780764_roles_json_fields.js`).

## Goal

Signed-in users get a course catalog with subscribe/unsubscribe and a **My Courses**
section; users gain **multi-role** identities (special role `admin`); courses can be made
**private** and opened to selected roles or named individuals; **admins manage all of it
through in-app pages**. Access config lives in PocketBase (user decision), not course files.

## Design decisions (resolved with user)

1. **Access config in PocketBase**, managed via Brillante admin pages (no file edits/reloads).
2. **Subscription = personal "My Courses" list only.** Access is governed independently by
   course privacy (public / roles / named grants). Subscribing never bypasses privacy;
   private courses must be granted before they can be subscribed.
3. **Multi-role users.** Stored as a multi-value `roles` text field on the PB `users`
   record (denormalized on purpose: PB API rules can reference `@request.auth.roles`
   but cannot join a separate user_roles collection — rule expressibility requires the
   field on the user record). Special value `admin`; other values are free-form
   (e.g. `instructor`, `student-fall26`), matched exactly against a course's `allow_roles`.
4. **First admin via config bootstrap.** `brillante.toml` gains
   `[pocketbase] bootstrap_admins = ["email", ...]`. At startup the server authenticates
   to PB as a superuser (env `BRILLANTE_PB_SUPERUSER_EMAIL` / `BRILLANTE_PB_SUPERUSER_PASSWORD`,
   same pattern `pocketbase/run.sh` already uses) and idempotently ensures each listed
   account holds role `admin` (auto-creating the user if the email has no account? **No** —
   only promote *existing* accounts; log a warning listing emails with no account).
   Bootstrap retries in the background until PB is reachable; superuser credentials are
   never persisted beyond the client object and never used for request handling.

## Data model (new PB migration)

New migration file `pocketbase/pb_migrations/<ts>_roles_access_subscriptions.js`
(never edit the existing one; down-migration reverts everything):

- **`users`** — add `roles` multi-value text field (max 16 entries, each ≤ 64 chars,
  lowercase-normalized server-side when written by Brillante). Rule changes:
  > **Resolved during debug (2026-10-12):** PB 0.23+ removed multi-select from
  > text fields, so `roles` is a **JSON array field** and API rules use a
  > mirrored boolean **`is_admin`** (the documented fallback above) — PB 0.40.4
  > filters cannot exactly match JSON array elements (`?=` never matches, `~`
  > is substring). Migration `1791780764_roles_json_fields.js` applies this.
  - `listRule` / `viewRule`: `id = @request.auth.id || @request.auth.roles:each = "admin"`
    (admins manage users; everyone reads self — required by existing role fetch + `/auth/me`).
  - `updateRule`: `@request.auth.roles:each = "admin"` (admin-only record edits; this is
    what prevents self-role-escalation. No user self-edit UI exists today, so nothing breaks).
  - `createRule` stays `""` (open self-registration, unchanged).
- **`course_access`** — one row per configured course: `course` (unique text slug, pattern
  `^[a-z0-9]+(?:-[a-z0-9]+)*$`), `visibility` (select: `public` | `private`), `allow_roles`
  (multi text). List/view rules `""` (world-readable: the anonymous catalog must know which
  courses are public; leaks only visibility + role vocabulary, never emails).
  Create/update/delete: `@request.auth.roles:each = "admin"`.
- **`course_grants`** — `user` (relation → users, cascadeDelete), `course` (text slug).
  Unique `(user, course)`. List/view: `user = @request.auth.id || @request.auth.roles:each = "admin"`
  (users see only their own grants; admins manage all). Create/delete: admin-only.
  **Default when a course has no `course_access` row: public** (backwards compatible —
  servers without PB or without records behave exactly as today).
- **`course_subscriptions`** — `user` (relation), `course` (text slug), `created` autodate.
  Unique `(user, course)`. All rules user-owned (`user = @request.auth.id`); createRule
  additionally `@request.body.user = @request.auth.id` (same pattern as `progress`).

> Verify the `:each` modifier works on multi-value *text* fields under PB 0.40.4 rules
> (`@request.auth.roles:each = "admin"`); documented fallback if not: keep a mirror
> single-value boolean `is_admin` field maintained by Brillante writes + `:each` only in
> Brillante-side logic. Do not ship rules that silently never match.

## Server plumbing

### `src/pb.rs` additions (hand-rolled REST client stays)

- `AuthUser` gains `roles: Vec<String>` (parsed from user records; default empty).
- `superuser_auth(email, password)` → token (POST `/api/collections/_superusers/auth-with-password`).
- `bootstrap_admins(token, emails)`: for each email, find user by filter
  `email = "<escaped>"`; if found and `roles` lacks `admin`, PATCH `roles` (append).
- `fetch_user(token, id)` → record incl. `roles` (viewRule: self).
- `list_course_access(token: Option<&str>)` → all `course_access` rows (anonymous call).
- `list_subscriptions(token)`, `subscribe(token, user_id, course)`,
  `unsubscribe(token, course)` (find-by-filter + delete, idempotent like uncomplete).
- `list_grants(token)` (self grants for users; admin pass-through for admin UI).
- Admin ops: `upsert_course_access(token, slug, visibility, allow_roles)`,
  `delete_grant(token, id)`, `create_grant(token, user_id, slug)`,
  `list_users(token, filter, page)` (email search), `update_user(token, id, roles)`.
- Reuse `filter_quote` + slug validation (`valid_slug` moves from `progress.rs` into a
  shared helper, e.g. `crate::pb` or `crate::web::util`).

### Access resolution (`src/web/access.rs`, new)

- `AccessMap`: slug → `CourseAccess { visibility, allow_roles }`, fetched from PB and cached
  in `AppState` behind a mutex with **30s TTL** (visibility changes propagate within 30s;
  role/grant checks are never cached).
- Pure decision fn (colocated unit tests):
  `can_view(slug, Option<&(roles, own_grant_slugs)>, admin: bool, map) -> bool`:
  no record → public; `public` → true; `private` → admin ∥ roles∩allow_roles≠∅ ∥ granted.
- Per-request context for signed-in users: user record (`roles`, 1 call) + own grants
  (1 call) + own subscriptions (1 call), fetched concurrently (`tokio::join!`); anonymous
  → none. Catalog additionally uses the cached `AccessMap`.
- Rendering degrades like progress does today: on PB unreachable, render as anonymous
  (deny private, hide subscribe UI) — never error the page.

### Enforcement points (all must be gated)

- `pages::home` (catalog): filter cards by `can_view`; admins see everything; private
  courses a user can view get a "Private" chip.
- `pages::course_page` + `pages::lesson_page`: not viewable → for **anonymous** redirect to
  `/account?next=<path>`; for **signed-in without access** → existing 404 page (existence hidden).
- `content_files::serve`: same check on the `{course}` segment — private-course images must
  not leak. Auth-disabled servers: only no-record (public) courses pass.
- `/admin/*` pages: require `Authed` + `admin` role (else 404, same hiding semantics).
- `POST` subscribe: re-check `can_view` server-side before creating the row.

## Routes & UI

- `POST /my/subscriptions` — form fields `course`, `action` (`subscribe`|`unsubscribe`),
  `next`; plain form POST + redirect (works with JS off, consistent with `/auth/*`).
- Home page (`pages::home`) becomes the catalog: when signed in, a **My Courses** section
  (subscribed & visible courses, card shows lesson-count + completed count like today's
  outline) above the full catalog; every card gets a subscribe/subscribed toggle button.
  Anonymous: public catalog only, no subscribe UI.
- Course outline hero gains the same subscribe/unsubscribe toggle.
- Admin pages (leptos SSR views, plain forms):
  - `GET /admin/users` — paginated user list (email, name, roles), email search box,
    per-user add/remove role form (text input for role name, `admin` included).
  - `GET /admin/courses` — registered courses with effective visibility + a settings form
    per course: visibility select, `allow_roles` (comma-separated text → normalized list),
    and grants management (email input → grant; existing grants listed by expanded email
    with revoke buttons; PB `expand=user` on the grants list).
  - Nav header gains an **Admin** link only when the session user has the `admin` role.
  - CSRF posture unchanged from `/auth/*`: SameSite=Lax + POST forms.

## Config changes

- `brillante.toml`: `[pocketbase] bootstrap_admins = [...]` (optional, empty default).
- `PocketBaseConfig` gains `bootstrap_admins: Vec<String>`. Superuser creds come only from
  env (`BRILLANTE_PB_SUPERUSER_EMAIL`, `BRILLANTE_PB_SUPERUSER_PASSWORD`); never from the
  committed toml. Missing env + non-empty bootstrap list → warning diagnostic, no bootstrap.

## Task list

1. **PB migration** — collections + users field/rules per above; verify `:each` rule
   behavior against the live dev PB (curl a private `course_access` write as admin and as
   plain user); document the exact rule syntax that works.
2. **pb.rs client methods** + `valid_slug` helper relocation + unit tests (JSON parsing
   tests colocated, following existing `parse_auth_methods` test pattern).
3. **Bootstrap task** — config plumbing + startup background upsert with retries; warning
   diagnostics for unreachable PB / unknown emails.
4. **`access.rs`** — AccessMap cache + `can_view` + per-request context fn + unit tests.
5. **Enforcement** — gate `home`, `course_page`, `lesson_page`, `content_files::serve`,
   `/admin/*`; 404/redirect semantics; degrade-on-unreachable.
6. **Subscriptions** — collection client methods + `POST /my/subscriptions` +
   home/catalog/outline UI + access re-check on subscribe.
7. **Admin UI** — `/admin/users`, `/admin/courses` pages + form handlers.
8. **Tests** — `tests/web.rs`: fixture asserts (auth disabled ⇒ no subscribe UI, no-record
   course visible, unknown private slug still 404s; admin pages 404 without PB).
   `tests/pocketbase_live.rs` (skipped without `POCKETBASE_TEST_URL`): bootstrap admin,
   signup, subscribe/unsubscribe idempotence, private-course anonymous redirect /
   signed-in 404 / grant-then-200, role-gated access, admin role edit, non-admin forbidden
   from admin writes at the PB layer.
9. **Docs** — AGENTS.md (new collections, bootstrap env vars, invariant update: static
   content serving is now auth-gated; catalog/subscription semantics) + README commands.

## Risks / notes

- `:each` rule syntax on multi-text fields is the one unverified PB 0.40.4 assumption —
  task 1 validates it before anything builds on it.
- Per-request PB round-trips rise to ~3 (concurrent) + cached map; acceptable on localhost,
  noted as future optimization only.
- `user_roles`-style separate collection was rejected: PB rules cannot reference it,
  which would make admin-gated API rules inexpressible.
- Visibility TTL (30s) means privacy changes lag ≤30s on cached pages — documented,
  acceptable for v1.
- `progress` rows and subscriptions for courses a user loses access to are retained
  (harmless; hidden from UI).

## Out of scope (v1)

- Approval workflows / paid enrollment, email invitations, per-lesson visibility,
  course search, admin analytics/dashboards, unlisted visibility tier.

---

# Phase 2: Notebooks (per-user course notes)

## Goal

Signed-in learners get **one notebook per course**: markdown notes written per
lesson, supporting **LaTeX** (`$...$` / `$$...$$`) and **shorthand links** —
`[[lesson-slug]]` to any lesson in the course, plus `@here` autocomplete that
links to the nearest section of the lesson the note was written from.

## Design decisions (resolved with user)

1. **Audience: per-user learner notes** (like the scratchpad, but persisted
   markdown). Not author-authored content; nothing in course directories.
2. **Granularity: one notebook per (user, course)**, organized as per-lesson
   entries. One entry per (user, course, lesson) — the lesson notes model.
3. **Shorthand: `[[lesson-slug]]`** (wiki-style, lesson title auto-filled as
   link text) **plus `@here` autocomplete** that links to the nearest section
   of the current lesson.
4. **Editing: plain textarea + preview** (no editor framework, no CDN editor
   dependency), consistent with the scratchpad and `/auth/*` progressive
   enhancement posture.

## Data model (new PB migration `pocketbase/pb_migrations/<ts>_notebook_entries.js`)

- **`notebook_entries`** — `user` (relation → users, cascadeDelete), `course`
  (text slug, pattern same as subscriptions), `lesson` (text slug), `anchor`
  (text, optional — heading id captured client-side at save time), `here_title`
  (text, optional — section heading text), `body` (text, required, markdown),
  `created`/`updated` autodate. Unique index `(user, course, lesson)`.
- All rules strictly user-owned, mirroring `course_subscriptions`:
  `user = @request.auth.id` on list/view/update/delete; createRule additionally
  `@request.body.user = @request.auth.id`.
- Notes live in PocketBase, never in course directories (course dirs stay
  read-only; `.toml`/dotfile serving invariants untouched).
- Storage is **raw markdown**; shorthand resolution and rendering happen at
  request time so links survive lesson renames and re-slug (a dead `[[slug]]`
  renders as a dead-link, it does not corrupt the stored text).

## Rendering pipeline (reuses `src/content/markdown.rs`)

- Server-side rendering only — the same escaped-HTML pipeline lessons use;
  **no author HTML is ever emitted** (platform invariant holds for notebooks).
- New pure preprocessing step before pulldown-cmark (walks events like
  `scan_widget_tags_in_markdown`, so fenced code blocks and inline code are
  immune):
  - `[[lesson-slug]]` → `[Lesson title](/course/<course-slug>/<lesson-slug>)`
    via ContentStore lookup; unknown slug → `dead-link` span (consistent with
    existing dead-link policy) + `tracing` warning (per-user content is not
    part of loader diagnostics).
  - `@here` → `[§ <here_title>](/course/<course-slug>/<lesson>#<anchor>)` using
    the entry's stored `anchor`/`here_title`; `@here` in an entry with no
    stored anchor renders as plain text.
  - Unit-testable pure function: `expand_note_links(body, course_slug, lesson, here, lessons) -> String`.
- **LaTeX**: `$inline$` / `$$display$$` work via the existing math pipeline;
  notebook pages include the `math` widget assets (KaTeX). No widget scanning:
  `<Widget>` tags in note bodies render as escaped literal text (notebooks are
  not lesson content and cannot reference `config.toml` instances).

## Editing UX

- **Lesson page gains a "Notes" drawer** (styled after the scratchpad panel),
  server-rendered only when auth is enabled AND the user is signed in:
  - The saved rendered entry (if any) above a textarea pre-filled with the raw
    markdown body (so JS-off save round-trips work — plain form POST).
  - **Save**: `POST /notes/{course}/{lesson}` (form: `body`, `anchor`,
    `here_title`) → redirect back to the lesson (SameSite=Lax posture as
    `/auth/*`). The client JS computes the nearest section heading (id + text,
    closest to caret / scroll position) and fills the hidden `anchor` /
    `here_title` fields; with JS off the server just stores them empty and
    `@here` stays inert text.
  - **Preview**: JS-only — `POST /api/notes/preview` with `{course, lesson,
    body, anchor, here_title}` returns the rendered HTML fragment, injected
    into the preview pane. Server renders it; client never renders markdown.
  - **Autocomplete** (`notes.js`): typing `@` pops the current lesson's section
    headings (read from the lesson DOM: h2/h3 with slugified ids) and inserts
    `@here`; typing `[[` pops the course's lessons (slug + title from a
    `data-lessons` JSON payload embedded in the drawer) and inserts
    `[[lesson-slug]]`. Arrow-key/Enter selection, Escape dismiss.
- **Notebook index page `GET /notes/{course}`** — requires Authed (redirect to
  `/account?next=` when signed out; 404 when PB is disabled): rendered entries
  in lesson order, each with the lesson title and an "Edit in lesson" link.
  Entries for lessons that no longer exist render with a "lesson removed" chip.
- Anonymous: no drawer, just a "Sign in to keep notes" link (rendered only when
  auth is enabled). PB unreachable → drawer hidden, page never errors.

## Routes & server plumbing

- `src/pb.rs`: `get_notebook_entry(token, user_id, course, lesson)`,
  `upsert_notebook_entry(...)` (upsert on unique index; duplicate treated as
  update, idempotent like `complete`), `list_notebook_entries(token, user_id,
  course)` (sorted by lesson order client-side).
- Handlers in `src/web/notes.rs` (new; router registration in `web/mod.rs`):
  `GET /notes/{course}`, `POST /notes/{course}/{lesson}`,
  `POST /api/notes/preview`.
- **Access gating**: every notes route re-checks `can_view` for the course
  (private courses gate notes like lessons); `course`/`lesson` slugs validated
  against the ContentStore before any PB call (same discipline as `progress`).
- Rendering context: a `MarkdownCtx` for notebook bodies (course slug, lesson
  list for link resolution) — built per request, no caching needed.
- CSS/JS ship as `src/web/notes.css` / `notes.js`, `include_str!`-ed in
  `assets.rs`, loaded only on pages that render the drawer or notebook page.

## Task list (Phase 2)

1. **Migration** `notebook_entries` (up + down), verify rules against live dev PB.
2. **pb.rs methods** + colocated JSON-parsing tests (existing pattern).
3. **Shorthand preprocessor** — pure fn + unit tests (linkification, `@here`,
   code-block immunity, unknown-slug dead-link).
4. **`notes.rs` handlers** + router wiring + access gating + degrade.
5. **Lesson-page drawer + notebook index page** (leptos views in `pages.rs` or
   `notes.rs`), textarea server-rendered with current body.
6. **`notes.js`/`notes.css`** — save, preview fragment injection, `@`/`[[`
   autocomplete; asset registration; `data-lessons` payload.
7. **Tests** — `tests/web.rs` fixture asserts (drawer absent when auth disabled;
   notebook page redirects signed-out); `tests/pocketbase_live.rs` (upsert
   idempotence, per-user isolation, slug validation, private-course gating).
8. **Docs** — AGENTS.md (new collection, routes, invariant note: notebook bodies
   go through the same escaped-HTML pipeline) + README.

## Phase 2 status: COMPLETE (2026-10-12)

All 8 tasks shipped and verified: 146 unit + 43 web + 7 pocketbase_live tests
green, clippy clean. Deviations/notes from implementation:

- Migration `1791822976_notebook_entries.js` uses **raw field objects** in the
  `Collection` constructor (the `new XField()` constructor form is silently
  dropped there in PB 0.40.4 — first attempt failed rule validation with
  "unknown field user"; existing Phase-1 migrations already use raw objects).
  Owner rules verified live: foreign-user create 400, cross-user list empty,
  duplicate (user, course, lesson) 400 → drives the upsert.
- `upsert_notebook_entry` takes a `NoteDraft` struct (clippy too-many-args).
- `expand_note_links` emits `[title](<file_name>)` links so the existing
  `resolve_link`/dead-link machinery does the URL rewriting — no new link
  policy needed; unknown slugs target the sentinel `notebook-missing.md`.
- Widget suppression is `MarkdownCtx::widgets: false` (renders raw-HTML events
  as escaped text) rather than a parallel renderer.
- Anchor validation is **silent-drop** (clear anchor+title, keep the note) per
  the risk note "reject/ignore unknown ids" — ignoring was chosen so a user
  never loses a draft to a vanished heading.
- Drawer is inline (fully usable with JS off); `notes.js` adds
  `body.notebook-drawer` to convert it to the fixed panel with toggle.
- Browser sign-off passed: `[[`/`@` autocomplete, Preview fragment, save →
  `?notes=saved` status, index listing with lesson + `#anchor` links, and the
  anonymous index → `/account?next=/notes/<course>` redirect.

## Risks / notes

- `@here` resolution depends on client-supplied anchor; the server validates it
  against the lesson's heading ids before storing (reject/ignore unknown ids) so
  clients cannot forge arbitrary targets.
- One entry per lesson means notes are append/edit within a lesson, not
  arbitrary many documents — deliberate granularity choice (v1).
- Per-request PB round-trips grow again (entry fetch + access context);
  acceptable on localhost, same future-optimization note as Phase 1.

## Out of scope (Phase 2)

- Sharing/exporting notebooks, multiple notebooks per user per course, image
  uploads in notes, collaborative editing, notebook widgets/interactive config.

---

# Debug task: missing SSO buttons

## Symptom

The "Continue with Google" / "Continue with GitHub" buttons are not showing
where expected. They render only on `GET /account` (the signed-out sign-in
panel — `src/web/auth.rs:102-132`) when PocketBase reports enabled OAuth2
providers; there is no other page that emits them by design.

## Rendering chain (verified from code)

1. `src/web/auth.rs:102` — buttons render only when the visitor is **signed
   out** AND `pb.oauth_providers()` returns a non-empty list. Signed-in users
   get an intentionally empty block (`<span hidden>`).
2. `src/pb.rs:188` — `oauth_providers()` calls
   `GET {pb}/api/collections/users/auth-methods`. **Any** transport/HTTP error
   → `Err` → caller maps to `Vec::new()` with only a `tracing::debug!` line
   (`"oauth providers unavailable on /account: ..."`), invisible under the
   default `RUST_LOG`.
3. `src/pb.rs:955` — `parse_auth_methods` extracts `oauth2.providers`
   (tolerating a legacy `authMethods` envelope) and **silently skips any
   provider entry with an empty `authURL`** — a disabled/misconfigured PB
   provider therefore produces zero buttons with no warning.
4. Providers come from `pocketbase/pb_hooks/auth0.pb.js`, which syncs two
   slots onto the `users` collection (`oidc` = Google, `oidc2` = GitHub) from
   env `AUTH0_DOMAIN` / `AUTH0_CLIENT_ID` / `AUTH0_CLIENT_SECRET` on every PB
   start. If any variable is unset, the hook returns early and leaves PB
   as-is (password-only unless previously synced).
5. `pocketbase/run.sh` forwards those `AUTH0_*` vars into the podman container
   and mounts `pocketbase/` (hooks included). It recreates the container each
   run; state persists in `pocketbase/pb_data`.

Static evidence already gathered: the Rust side is fully wired (routes
registered in `src/web/mod.rs:39-40`; unit tests for `parse_auth_methods` and
`start_href` pass in-tree), and the dev `pocketbase/pb_data/data.db` still
contains previously-synced provider rows (Auth0 `btapi.auth0.com`, Google +
GitHub connections) — so hypothesis B below only bites on a wiped/reseeded
pb_data or a container started without the env vars.

## Root-cause candidates (ranked)

- **A. PB unreachable** — container not running, or the Rust server points at
  the wrong port; `oauth_providers()` fails and only a debug log records it.
- **B. Hook didn't sync** — `AUTH0_*` env vars were unset when `run.sh` last
  started the container → `users.oauth2` disabled / provider list empty on
  the served db.
- **C. Stale server binary** — the running `cargo run` process predates the
  Phase-1 OAuth button code; HTML has no `.oauth-buttons` at all regardless of
  PB state (Rust changes need a rebuild).
- **D. Response-shape mismatch** — PB 0.40.4's auth-methods JSON matches
  neither the legacy `authMethods` envelope nor the top-level `oauth2` shape
  `parse_auth_methods` expects; every entry gets skipped (also covers entries
  with empty `authURL` being silently dropped).
- **E. Expected behavior** — visitor already signed in (buttons hidden by
  design), or CSS hides the rendered block (`.oauth-intro`/`.oauth-buttons`).

## Ordered diagnostic steps (each step disambiguates)

1. **Rule out E (state/CSS) first** — in a browser (playwright-cli, step 7):
   clear cookies, open `http://127.0.0.1:3000/account`, inspect the raw HTML.
   If `div.oauth-intro` / `div.oauth-buttons` is absent entirely → provider
   list was empty at render time (go to 3). If present but invisible → CSS
   issue in `site.css` (go to fix 6 directly).
2. **Rule out C (stale build)** — compare the running process start time /
   binary mtime against the last commits touching `src/web/auth.rs`,
   `src/pb.rs`, `src/web/oauth.rs`. If stale, rebuild + restart `cargo run`,
   then re-check.
3. **Make the silent failure visible** — restart the server with
   `RUST_LOG=brillante=debug` and reload `/account`:
   - `oauth providers unavailable on /account` present → the PB call is
     failing → go to 4.
   - absent while buttons still missing → PB answered but parsing dropped
     everything → go to 5.
4. **Probe PB directly** — `curl -s http://127.0.0.1:8090/api/health`, then
   `curl -s http://127.0.0.1:8090/api/collections/users/auth-methods | jq`.
   - Connection refused / unhealthy → hypothesis A: start PB via
     `./pocketbase/run.sh` (export `AUTH0_*` first or rely on `.env`), then
     repeat 3-4.
   - `oauth2.enabled: false` or empty `providers` → hypothesis B: check
     `podman logs brillante-pocketbase` for the `auth0 hook:` messages. Re-run
     `run.sh` with `AUTH0_DOMAIN`/`AUTH0_CLIENT_ID`/`AUTH0_CLIENT_SECRET` set
     and confirm the container mounts the repo's `pocketbase/` dir (hook file
     visible inside). Then repeat 4.
   - Two providers present with non-empty `authURL`, `state`, `codeVerifier`
     → PB is fine; the problem is server-side parsing → go to 5.
5. **Cross-check what the server received** (only if 4 looked correct) —
   compare the curl JSON against `parse_auth_methods`' accepted shapes; if the
   real response has a different envelope or entries without `authURL`,
   capture the actual JSON as a new fixture in the `parse_auth_methods_*`
   unit tests (`src/pb.rs` tests module) → hypothesis D fix.
6. **Fix + regression-test** the identified cause:
   - A: environment/setup, no code change (consider promoting the `/account`
     empty-provider case from `debug!` to `warn!` — a one-line change that
     makes this failure class visible under the default `RUST_LOG`).
   - B: environment/setup; confirm hook log line `auth0 hook: synced oidc
     oauth2 providers for <domain>` appears on container start.
   - C: rebuild; no code change.
   - D: extend `parse_auth_methods` (tolerance, or an explicit warning when
     an `oauth2` block exists but yields zero usable providers) + unit test
     with the captured fixture.
   - E/CSS: fix the offending rule in `site.css`.
7. **Frontend verification with playwright-cli** (required sign-off):
   - `playwright-cli open http://127.0.0.1:3000/account` on a fresh profile;
     `playwright-cli find "Continue with"` must match both buttons.
   - Click the Google button → must 302 to the Auth0 authorize URL containing
     `connection=google-oauth2` and
     `redirect_uri=<public_url>/auth/oauth/oidc/callback`; confirm a
     `brillante.oauth` httpOnly cookie was set (`playwright-cli cookie-list`).
   - Repeat for GitHub (`oidc2`).
   - Full E2E through Auth0 requires real credentials; stopping at the
     provider redirect + flow-cookie assertions proves the buttons and start
     handler are wired. Complete a real login once manually to confirm the
     callback promotes the session cookie and lands on `next`.

## Acceptance criteria for closing the debug task

- Signed-out `/account` shows both SSO buttons; signed-in `/account` shows
  none (by design).
- The server-visible failure mode (PB unreachable, or zero usable providers
  while auth is enabled) is logged at a level visible with the default
  `RUST_LOG`, with a unit test covering any parser change.
- `cargo test` and `cargo clippy` clean; playwright steps in 7 pass against
  the local stack.
- Root cause recorded in this file's status note.

## Out of scope for this debug task

- Adding SSO buttons to pages other than `/account` (if the expectation was
  buttons on e.g. the home header or a dedicated /login page, that is a
  separate UI feature — flag it, don't fix it here).
- New providers beyond the two Auth0-backed slots; changes to the
  `brillante.oauth` flow-cookie semantics.
- Phase 2 (Notebooks) work proceeding in parallel.

## Resolution (2026-10-12)

**Root cause: hypothesis A** — the PocketBase container was not running and
its local image (`brillante-pb:0.40.4`) had been pruned, so `run.sh` failed
trying to pull it from Docker Hub. Hypotheses B–E ruled out by evidence: the
hook synced both providers on container start (`auth0 hook: synced oidc oauth2
providers for btapi.auth0.com`), the served `auth-methods` JSON matched
`parse_auth_methods`' shape exactly (top-level `oauth2.providers`, non-empty
`authURL`/`state`/`codeVerifier`), the server was freshly built, and the
buttons rendered once PB answered.

**Latent bug found while verifying the real-login sign-off (fixed in the same
pass):** PB 0.23+ removed `maxSelect` from text fields, so migration
`1791307800` silently created `users.roles` and `course_access.allow_roles` as
single-value text. Records then serialized `"roles": ""`, which failed to
decode into `Vec<String>` — **every** auth response broke: password login
(422), the OAuth callback (`oauth decode: error decoding response body` seen
in logs), and role writes. Fixed by:

- New migration `1791780764_roles_json_fields.js`: `roles`/`allow_roles` →
  JSON fields; adds mirrored boolean `is_admin`; rewrites all admin API rules
  from `@request.auth.roles ?= "admin"` to `@request.auth.is_admin = true`
  (verified live on PB 0.40.4: `?=` matches nothing on JSON arrays and `~`
  matches substrings — `["superadmin"]` would match `"admin"` — so the plan's
  documented `is_admin` fallback is the only safe rule expressible).
- `src/pb.rs`: lenient `string_vec_lenient` deserializer for `AuthUser.roles`,
  `RecordJson.roles`, `CourseAccessRow.allow_roles` (array | null | legacy
  comma-string); `roles_payload` keeps `is_admin` in sync on every role write
  (`update_user_roles`, hence bootstrap + admin UI).
- `src/web/auth.rs`: the silent `/account` degradation cases (`debug!` on
  provider fetch failure, no log at all for zero usable providers) promoted to
  `warn!` per fix step 6-A.

**Verification:** `cargo test` 128 unit + 35 web + 5 pocketbase_live (against
the dev PB) all pass; `cargo clippy` clean. Live rule matrix verified: admin
write to `course_access` 200, plain-user write 400, plain-user viewing another
user 404, signup submitting `is_admin` 400. Playwright sign-off: signed-out
`/account` shows both buttons; clicking Google/GitHub 302s to Auth0 with
`connection=google-oauth2`/`connection=github` and
`redirect_uri=.../auth/oauth/{oidc,oidc2}/callback`, sets `brillante.oauth`
httpOnly/SameSite=Lax (decoded payload carries provider/state/verifier/next);
signed-in `/account` shows none. Password signup+login round-trips with a
`brillante.session` cookie. **Remaining manual step:** complete one real
Google/GitHub login through Auth0 (requires the user's provider credentials)
to confirm the callback promotes the session cookie and honors `next`.
