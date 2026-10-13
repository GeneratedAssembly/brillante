# Brillante

A content-delivery platform for interactive courses, in the spirit of
brilliant.org. Courses live in ordinary directories of markdown; the platform
renders them server-side, with one crucial addition: a special `<Widget />`
tag that brings interactive elements into lessons.

Everything else you might paste as HTML is **not rendered** — Brillante emits
only its own markup, so lesson authors can never inject raw HTML.

## Quick start

```sh
cargo run
# then open http://127.0.0.1:3000
```

The example course in `content/getting-started/` is registered via
`brillante.toml` and demonstrates the content model. It is the only course
tracked in the repository; `.gitignore` ignores every other directory under
`content/`, so your own courses stay local until you choose to publish them.

Accounts and cross-device progress sync are optional and backed by PocketBase —
see [Accounts & progress sync](#accounts--progress-sync). Out of the box the
server runs without them.

### CLI

```
brillante [OPTIONS]

-c, --config <path>    config file (default: brillante.toml if present)
    --host <addr>      bind address (default 127.0.0.1)
-p, --port <port>      bind port (default 3000)
    --course <dir>     register an extra course directory (repeatable)
```

If no config file exists, Brillante auto-discovers subdirectories of
`./content` that contain markdown or a `course.toml`.

## Registering course directories

```toml
# brillante.toml
[server]
host = "127.0.0.1"
port = 3000

courses = ["content/getting-started", "content/another-course"]
```

Each registered directory is one course. Reload after editing registrations or
content:

```
curl -X POST http://127.0.0.1:3000/admin/reload
curl http://127.0.0.1:3000/admin/diagnostics   # warnings and errors from loading
```

## Course directory layout

```
content/my-course/
├── course.toml        optional course metadata
├── config.toml        optional widget instance configs
├── 01-intro.md        lessons: every .md file is a lesson
├── 02-loops.md
└── images/            served at /content/<course-slug>/images/...
```

### `course.toml`

```toml
title = "My Course"          # falls back to the directory name
slug = "my-course"           # falls back to the slugified directory name
description = "What you will learn"
accent = "#0ea5e9"           # hex color used as the course theme accent

# Optional lesson grouping shown on the course outline page. URLs, lesson
# numbering, progress, and the pager stay flat — sections only affect the
# outline rendering.
[[sections]]
title = "Rays"               # required
slug = "rays"                # default: slugified title; duplicates get -N + warning
description = "Geometric optics"  # optional lede under the section title
```

### Lesson frontmatter

Lessons start with YAML (`---`) or TOML (`+++`) frontmatter:

```markdown
---
title: Looping Patterns        # required-ish: falls back to the file name
description: Short summary     # shown in course outline and lesson lede
slug: looping-patterns         # default: file name without order prefix
order: 2                       # sorts lessons; ties broken by file name
draft: false                   # true = excluded from the course entirely
section: rays                  # optional [[sections]] slug; unknown → warning + ungrouped
---

Lesson body in plain markdown...
```

URLs are derived from file names with ordering prefixes stripped:
`02-loops.md` → `/course/<slug>/loops`. Relative markdown links to sibling
`.md` files are rewritten into lesson URLs, and relative image sources are
served from the course directory.

### Lesson body features

- Plain markdown: **bold**, *italics*, `inline code`, ~~strikethrough~~,
  lists, task lists, tables, blockquotes, and fenced code blocks.
- `$inline$` and `$$display$$` math render client-side via KaTeX (falling
  back to raw TeX text offline).
- Headings get slugified `id` attributes for anchor links.
- A relative link matching no lesson file renders as a visible dead link.
- Raw HTML is dropped entirely — the `<Widget />` tag below is the only
  markup that ever reaches the page.

## The `<Widget />` tag

```html
<Widget type="quiz" id="com.github.richbai90.quiz_basics" />
```

- **`type`** — which widget kind the renderer loads from its registry.
- **`id`** — the instance key. The server looks up the widget's configuration
  in `config.toml` located in the same directory as the course files.

The tag must be a single self-closing tag. Widgets are block-level; one
dropped mid-paragraph cleanly splits the paragraph. Multiple attributes may
appear in any order, with single or double quotes.

No other HTML is rendered: raw HTML blocks, inline tags, and comments are
dropped (visible in logs and `/admin/diagnostics` as debug entries). Widget
tags inside fenced code blocks are treated as literal text.

### `config.toml`

```toml
[widgets."com.github.richbai90.quiz_basics"]
question = "What does the type attribute select?"
options = ["The widget kind", "The URL of a server"]
answer = 0
explanation = "type names the kind; id looks up this table."
```

The id is matched against `[widgets."<id>"]` tables. A top-level
`[<id>]` table also works. A widget whose id has no entry (or whose config
fails validation) renders a visible error card instead of failing the page.

## Built-in widget kinds

All configs live in `config.toml`; the server converts TOML → JSON and each
widget validates its keys (bad values render a visible error card). Each kind
ships a small script and stylesheet that a page loads only when that kind
appears.

| kind | config keys |
|---|---|
| `quiz` | `question` (string, req), `options` (non-empty string array, req), `answer` (0-based index, default 0), `explanation` (optional). JS grades clicks and reveals the explanation after the correct answer. |
| `slider` | `label` (default "Value"), `min`/`max` (req, finite, `max > min`), `step` (default 1; the readout's decimal places follow the step), `initial` (default min, clamped), `unit`, `display` (template with `{value}`, or `{result}` — evaluated from `readout` — with Python-style precision specs like `{value:0.2f}`; overrides `unit`). A slider that adjusts nothing must still show its value; prefer a `{result}` display so moving the slider updates a computed quantity. |
| `plot` | `title`, `xlabel`, `ylabel` (all optional); data via `points = [[x,y],...]` (single series) or `series = [{name, color, points},...]`; `color` must be hex (palette fallback); canvas drawn by JS. |
| `flashcard` | `front`, `back` (both req). Click flips the card. |
| `math` | `tex` (req, KaTeX), `label` (optional), `definitions` (optional non-empty array of `{symbol, meaning, link?}`; `link` must start with `/` or `#`; renders as a collapsed definitions panel under the equation). |
| `stepper` | `label` (optional), `steps` (req, ≥ 2) of `{title?, tex?, text?, note?}` — each step needs `tex` or `text`. Server renders all steps; JS reveals one at a time (Prev/Next buttons, arrow keys, step counter) with KaTeX rendering per step. |
| `interactive` | Client-evaluated curve with parameter sliders: `expression` (req — safe DSL: `+ - * / ^`, parens, variable `x`, `pi`, functions `exp log sqrt abs min max pow sin cos`), `domain = [min, max]` (req), optional `ydomain`, `parameters = [{name, label?, min, max, step?, initial?}]` (step defaults to range/100, initial to the midpoint), `samples` (16–2000, default 240), `title`/`xlabel`/`ylabel`, `xslider` (bool; defaults on when the readout references `x`/`y` — adds an independent-variable slider), `readout` (template with `{x}`, `{y}` — the expression value — or `{paramname}` substitution, all with Python-style precision specs like `{x:0.2f}`). The server parses the expression into a JSON AST; the client never receives source text to `eval`. |
| `compare` | Two side-by-side equation panels: `left`/`right`, each `{title?, tex (string or string array, req), note?}`; titles default "Original"/"Revised". Legacy widget kept for older content. |
| `acknowledge` | `statement` (req), `button_label` (default "I acknowledge"), `done_message` (default "Acknowledged"). Checkbox + button; the click persists to localStorage (`brillante.ack.v1`, keyed by widget id) and dispatches `brillante:ack`. Courses containing any `acknowledge` tag get an automatic site-wide banner (course outline + every lesson) linking to the lesson with the widget; the banner stays up until one of the ids is acknowledged. |
| `resource-link` | Outbound card link to external material: `title` (req), `url` (req, absolute http(s)), `source` (req, e.g. "MIT OpenCourseWare"), `description` (optional), `kind` (optional: `video\|pdf\|assignment\|exam\|web`, default `web` — affects icon and label). Opens in a new tab. |

`$...$`/`$$...$$` math in lesson markdown reuses the `math` widget's KaTeX
loader automatically — you only need a `math` *widget* when you want a label,
a definitions panel, or a named instance.

## Adding a widget kind

A widget kind is one Rust render function plus a `.js`/`.css` pair, registered
in one place:

1. **`src/widgets/<name>.rs`** — implement
   `pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError>`.
   Read and validate every config key through the `WidgetInstance` accessors
   (`req_str`, `opt_str`, `req_f64`, `opt_f64`, `opt_usize`, `req_arr`,
   `opt_arr`, `opt_table`); on a bad value return
   `WidgetError::Invalid("clear message")` — the message is shown in the error
   card and logged, never a panic. Build static markup with leptos `view!` and
   wrap it in `widget_shell("<name>", &w.id, serialize_config(w.config.as_ref()), inner)`,
   which emits the root element with `data-widget-type`/`data-config`.
2. **`src/widgets/<name>.js` and `<name>.css`** — vanilla JS in an IIFE: on
   DOMContentLoaded, find `[data-widget-type="<name>"]`, read the config JSON
   from `data-config`, mount behavior, and guard with `el.dataset.mounted` so
   double-init is a no-op. No client framework, no `eval`/`new Function`.
3. **Register** the kind in `WidgetRegistry::with_builtins()` in
   `src/widgets/mod.rs`:

```rust
registry.register(WidgetDef {
    name: "my-widget",
    render: my_widget::render,        // fn(&WidgetInstance) -> Result<AnyView, WidgetError>
    script: include_str!("my-widget.js"),
    styles: include_str!("my-widget.css"),
});
```

Assets are `include_str!`d at compile time and served at
`/widgets/<kind>/script.js` and `/style.css` — a page loads them only when the
kind appears in its lesson, and changes require a rebuild.

## Guarantees

- **No author HTML is ever emitted.** The markdown pipeline renders semantic
  elements itself and text is always escaped.
- **Failures are contained.** An unknown widget type, missing config, or bad
  config renders an error card; the rest of the lesson is unaffected.
- **Diagnostics, not silence.** Load warnings (untitled lessons, duplicate
  slugs, invalid colors, bad frontmatter) are logged and listed at
  `/admin/diagnostics`.

## Routes

| Route | What |
|---|---|
| `/` | home: signed in → My Courses dashboard with progress + continue; signed out → course catalog |
| `/browse` | all viewable courses + search (`?q=`), matching course titles/descriptions and lesson titles |
| `/account` | sign in / create account; signed in → profile (roles, member-since, stats, keep-going) |
| `/course/{slug}` | course outline with progress |
| `/course/{slug}/{lesson}` | lesson page |
| `/widgets/{kind}/script.js`, `/widgets/{kind}/style.css` | per-kind assets |
| `/content/{course}/{path}` | files from the course directory (`.toml` and dotfiles blocked) |
| `/assets/site.css`, `/assets/site.js`, `/assets/favicon.svg` | platform assets |
| `/assets/scratchpad.css`, `/assets/scratchpad.js` | scratch pad assets (loaded on course pages only) |
| `POST /auth/login`, `/auth/signup`, `/auth/logout`, `GET /auth/me` | account endpoints |
| `GET /auth/oauth/{provider}` + `/callback` | SSO sign-in (providers reported by PocketBase) |
| `POST /my/subscriptions` | subscribe/unsubscribe (form: `course`, `action`, `next`) |
| `GET/POST /api/progress` | signed-in lesson progress (owner-scoped) |
| `GET /notes/{course}` | your notebook index for a course (signed in) |
| `POST /notes/{course}/{lesson}` | create/replace your note for a lesson |
| `POST /api/notes/preview` | render a draft note to an HTML fragment |
| `POST /api/tutor/chat` | signed-in tutor question (proxied LLM call, lesson context attached) |
| `/admin` | admin landing (redirects to the user list; admins only) |
| `/admin/users`, `/admin/courses` | admin pages (only with the `admin` role) |
| `POST /admin/users/roles`, `POST /admin/users/delete` | role edits, account deletion (admin) |
| `/healthz`, `POST /admin/reload`, `/admin/diagnostics` | operations |

Search is server-side and privacy-gated: results are filtered through the
same course-visibility rules as the catalog, so a private course never
appears as a hit (not even by lesson title) for viewers without access.

## Accounts & progress sync

Accounts are backed by [PocketBase](https://pocketbase.io) and are **optional**:
with no `[pocketbase]` section in `brillante.toml`, the site is fully static and
"Mark complete" is stored client-side in `localStorage` (key `brillante.progress.v1`).

When a `[pocketbase]` section is present:

```toml
[pocketbase]
url = "http://127.0.0.1:8090"
secure_cookies = false   # set true when Brillante itself is served over HTTPS
```

signed-in visitors get server-backed progress that syncs across devices, while
anonymous visitors keep the `localStorage` fallback — the two never mix on a
page. The browser only ever talks to Brillante's own endpoints; the PocketBase
token lives in an `httpOnly` cookie the server forwards as a Bearer token, and
every progress read/write is scoped to the authenticated user by PocketBase API
rules. All of it works with JavaScript disabled (plain form POSTs + redirects).

### Admin role

Holding the `admin` role (a value in the user's `roles` list, mirrored to the
`is_admin` field PB rules use) unlocks the Admin area: user search, role
assignment, individual course grants, course visibility, and account deletion
(confirm checkbox required; you cannot delete your own account or the last
admin). To make an account admin by default, list it under
`bootstrap_admins` in the `[pocketbase]` config section and provide
`BRILLANTE_PB_SUPERUSER_EMAIL` / `BRILLANTE_PB_SUPERUSER_PASSWORD` in the
server environment — or just keep them in `pocketbase/.env` (gitignored):
`pocketbase/run.sh` sources it for the container and the Brillante server
reads the superuser variables from it at startup, so a plain `cargo run`
self-heals the role.

To run the local dev PocketBase (podman) and the end-to-end tests:

```sh
./pocketbase/run.sh            # start PB at http://127.0.0.1:8090 (schema auto-applies)
POCKETBASE_TEST_URL=http://127.0.0.1:8090 cargo test --test pocketbase_live
```

`cargo test` skips the live suite when that env var is unset, so the default
test run never needs a running PocketBase.

## Virtual tutor

Admins can enable an **Ask-the-tutor** drawer on lesson pages (Admin → Tutor):
an enable checkbox, an OpenAI-compatible **endpoint** (base URL), an **API
key**, and a **model** name (free text — the admin supplies the exact model
id for their endpoint). When enabled, signed-in visitors get a chat panel
whose requests are proxied by the server: the model receives the course
title/description, the lesson outline, and the current lesson body as context
(bounded), plus the conversation so far. Replies render as markdown with
LaTeX (`$...$`, `$$...$$`, and the `\(...\)`/`\[...\]` forms models tend
to emit), using the same KaTeX pipeline as lessons. The API key is stored in a hidden
PocketBase field — never returned by any API, readable and writable only with
the server's superuser credentials — and chat is gated by the same course
visibility rules as everything else. The drawer is JS-only (no server-
rendered fallback; it simply doesn't appear without JS).

## Lesson notebooks

Signed-in visitors get a **Note** drawer on every lesson page: one private
note per lesson, saved to your account (PocketBase), never visible to anyone
else. Notes are markdown + LaTeX using the same renderer as lessons — with
two shorthand links:

- `[[lesson-slug]]` → a link to a lesson in the current course (type `[[` in
  the drawer to pick one).
- `@here` → a link to the *section* you were reading (type `@` to pick a
  heading on the page; the anchor is captured when you save).

Everything else that isn't markdown is kept out, exactly as in lessons:
`<Widget>` tags in a note render as literal text and can never reference the
course's widget configs. The drawer works fully without JavaScript (plain
form save + server-rendered note); with JS you also get live Preview and the
`@` / `[[` autocomplete. `GET /notes/<course>` lists all your notes for a
course in lesson order.

## Scratch pad

Every course view (outline and lesson pages) has a pull-out Python console on
the right margin — a tab labeled "Scratch pad". It runs real Python
(Pyodide/WebAssembly) entirely in the browser: nothing is sent to the server
and code cannot touch the host. The runtime (~10 MB) downloads from the CDN on
first open and is cached afterwards; offline, the console stays disabled the
way KaTeX falls back to raw TeX. It speaks IPython conventions — `In [n]:` /
`Out [n]:` prompts, blank line ends a block, `Tab` completion, `↑/↓` history,
top-level `await`, auto-loading of bundled packages (numpy, pandas, …) — with
a Reset button for a fresh namespace. State lives only in the current page.

## Docker

`compose.yaml` runs the full stack — Brillante plus PocketBase with the repo's
migrations and hooks:

```sh
cp .env.example .env        # set PB_SUPERUSER_EMAIL/PASSWORD (required)
docker compose up -d --build
# Brillante at http://localhost:3000, PocketBase admin at http://localhost:8090/_/
```

The `pb-init` one-shot creates the PocketBase superuser on a fresh volume
(PocketBase 0.23+ no longer bootstraps one from env alone). Brillante reads
`deploy/brillante.compose.toml` (mounted as its config) and mounts `content/`
and `courses/` read-only — register your courses there and set
`bootstrap_admins` to your account's email; the startup bootstrap promotes it
to admin automatically, retrying for ~30 minutes so signing up first is fine.

## Development

```sh
cargo test      # unit + integration tests (no PocketBase required)
cargo clippy
cargo run       # serve the example course
```

Architecture: `src/content/` (store, frontmatter, markdown→views pipeline),
`src/widgets/` (registry + built-ins), `src/web/` (axum router, pages, assets,
auth/session/progress), `src/pb.rs` (PocketBase client), `src/render.rs` (leptos
SSR helper). Pages and widgets are leptos views rendered to HTML server-side;
there is no client-side Rust/WASM.
