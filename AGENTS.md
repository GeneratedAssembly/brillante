# AGENTS.md — Guide for AI agents working in this repository

This is **Brillante**, a content-delivery platform for interactive courses (in the
spirit of brilliant.org). Courses are plain directories of markdown; the Rust
server renders them, and a `<Widget />` tag is the *only* way lesson authors can
add interactivity or markup. Everything HTML-shaped that an author pastes into a
lesson is dropped — the platform emits only its own markup.

Two very different kinds of work happen here. Know which one you are doing:

1. **Platform work** — Rust code: content pipeline, widgets, web layer.
2. **Course work** — content files under `content/`: markdown lessons, TOML
   widget configs, widget-instance tuning.

## Commands

```sh
cargo run            # serve at http://127.0.0.1:3000 (reads brillante.toml)
cargo run -- --course content/crlb   # serve an extra course directory
cargo test           # unit tests (colocated) + integration tests (tests/web.rs)
cargo clippy
```

CLI flags: `-c/--config <path>`, `--host`, `-p/--port`, `--course <dir>` (repeatable).

While the server runs:

```sh
curl -X POST http://127.0.0.1:3000/admin/reload      # reload content from disk
curl http://127.0.0.1:3000/admin/diagnostics          # loader warnings/errors
```

Logs go through `tracing`; `RUST_LOG` filters them (default `brillante=info,info`).

**The verification loop for content work is:** edit files → `POST /admin/reload`
→ check `/admin/diagnostics` is clean → load the lesson page and confirm no
`widget-error` cards. Content changes need no rebuild; Rust changes do.

## Repo map

```
brillante.toml          server config: host, port, registered course dirs
content/
  getting-started/      reference course demonstrating the content model
  crlb/                 real course under development (see tasks/)
tasks/plan.md           current project plan (CRLB course rebuild) — read first
tasks/todo.md           per-task breakdown with acceptance criteria
src/
  main.rs               CLI parsing, tracing init, server startup
  config.rs             brillante.toml loading + content/ auto-discovery
  app.rs                AppState (content store + widget registry), router
  render.rs             leptos SSR → HTML string helper
  diag.rs               Diagnostic { warning, error } model
  content/
    mod.rs              ContentStore: loads course dirs, dedupes slugs
    course.rs           Course/Lesson model, course.toml + config.toml, slugify
    frontmatter.rs      YAML (---) / TOML (+++) frontmatter split + parse
    markdown.rs         pulldown-cmark → leptos views; widget tag scanning;
                        link rewriting; math rendering; HTML suppression
  widgets/
    mod.rs              WidgetRegistry, WidgetInstance config accessors,
                        widget_shell / error_shell, resolve()
    expr.rs             safe arithmetic-expression parser (AST) for `interactive`
    quiz|slider|plot|flashcard|math|compare|stepper|interactive|acknowledge|resource-link  → .rs + .js + .css
  web/
    mod.rs              axum router, reload + diagnostics endpoints
    pages.rs            catalog, course outline, lesson page (leptos views)
    assets.rs           site assets + per-widget script.js/style.css
    content_files.rs    static file serving from course dirs (sandboxed)
tests/web.rs            integration tests (self-contained: builds a fixture
                        course in a temp dir, asserts nothing about shipped content)
```

Stack: Rust (edition 2024 — let-chains are used), axum 0.8 (path syntax is
`/{param}`, not `/:param`), leptos 0.8 SSR + tachys, pulldown-cmark, serde,
toml, tracing. **There is no client-side Rust/WASM and no hydration** — pages
are strings rendered server-side; per-widget JS is plain progressive
enhancement with no framework.

## Course authoring (developing courses)

A course is one directory. Register it in `brillante.toml`
(`courses = ["content/my-course"]`) or pass `--course`; if no courses are
configured, subdirectories of `content/` containing `.md` or `course.toml` are
auto-discovered. Note: `brillante.toml` currently registers only
`content/getting-started`; serve the CRLB course with
`cargo run -- --course content/crlb` or add it to the config.

```
content/my-course/
├── course.toml        optional metadata (+ optional [[sections]] grouping)
├── config.toml        widget instance configs
├── 01-intro.md        every top-level .md file is a lesson (non-recursive)
├── 02-loops.md
└── images/            served at /content/<course-slug>/images/...
```

### `course.toml`

```toml
title = "My Course"      # falls back to directory name
slug = "my-course"       # falls back to slugified directory name
description = "..."      # course outline lede
accent = "#0ea5e9"       # hex (3/6/8 digits); invalid values are dropped with a warning

[[sections]]             # optional, repeatable; groups lessons on the outline page
title = "Rays"           # required; slug defaults to slugify(title)
slug = "rays"            # duplicate slugs get a -N suffix + warning
description = "..."      # optional lede under the section heading
```

Sections only change outline rendering: one `<h2 class="section-title">` +
optional lede + `<ol class="lesson-list">` per group. URLs, global lesson
numbering, the pager, and progress stay flat. Lessons with `section` unset (or
with an unknown slug — warned and ignored) render before the declared sections
with no heading; a declared section with no lessons warns.

### Lesson files and frontmatter

- File names drive URLs: `02-loops.md` → `/course/<slug>/loops`. The numeric
  order prefix is stripped. Lessons sort by frontmatter `order` (f64), ties
  broken by natural filename sort.
- Frontmatter is YAML (`---`) or TOML (`+++
`) with keys: `title` (falls back to
  file stem, with a warning — always set it), `description`, `slug` (duplicates
  get a `-2` suffix), `order`, `draft: true` (excludes the lesson entirely),
  `section` (slug of a `[[sections]]` entry in `course.toml`).
- A leading `# H1` identical to the frontmatter title is stripped from the body.

### Lesson body rules

- **Plain markdown only.** Raw HTML blocks, inline tags, and comments are
  dropped (logged as debug; visible idea in `/admin/diagnostics`). Never work
  around this — if a lesson needs structure, use markdown or a widget.
- Enabled markdown extras: tables, strikethrough, task lists, `$inline$` and
  `$$display$$` math (rendered client-side via KaTeX).
- Headings get slugified `id` attributes for anchor links.
- Relative links to sibling `.md` files are rewritten to lesson URLs (a link
  matching no lesson file renders as a `dead-link` span plus a warning).
  Relative image srcs are served from the course directory.
- `<Widget type="kind" id="com.example.some_id" />` — must be a **single
  self-closing tag**; `type` is case-insensitive; attributes may use single or
  double quotes. Widgets are block-level and split paragraphs cleanly. Widget
  tags inside fenced code blocks stay literal text.

### `config.toml` (widget instances)

```toml
[widgets."com.github.richbai90.my_widget_id"]
question = "..."
options = ["...", "..."]
answer = 0
```

- The tag's `id` is matched against `[widgets."<id>"]` tables in the course
  directory's `config.toml` (a bare top-level `[<id>]` table also works).
- **Conventions:** use reverse-DNS ids namespaced per course, e.g.
  `com.github.richbai90.crlb2.math_score`. Ids must be unique within the course.
- A missing entry, or a config that fails validation, renders a visible error
  card — the rest of the page is unaffected. That card is your signal that the
  lesson references an id with no (or an invalid) config entry; this drift
  between `.md` and `config.toml` is the most common content bug.
- `.toml` files and dotfiles under a course directory are never served over HTTP.

## Built-in widget kinds

All configs live in `config.toml`; the server converts TOML → JSON and each
widget validates its keys (bad values → error card + diagnostic). Each kind
ships a `.js`/`.css` pair loaded by a page only when that kind appears.

| kind | config keys |
|---|---|
| `quiz` | `question` (string, req), `options` (non-empty string array, req), `answer` (0-based index, default 0), `explanation` (optional). JS grades clicks and reveals the explanation after the correct answer. |
| `slider` | `label` (default "Value"), `min`/`max` (req, finite, max > min), `step` (default 1, > 0; decimal places of the readout follow the step), `initial` (default min, clamped), `unit`, `display` (template with `{value}`, or `{result}` computed from `readout`; precision specs like `{value:0.2f}` allowed; overrides `unit`), `readout` (safe expression over variable `x` — req when `display` uses `{result}`, and then `display` must contain it). |
| `plot` | `title`, `xlabel`, `ylabel` (all optional); data via `points = [[x,y],...]` (single series) or `series = [{name, color, points},...]`; `color` must be hex (palette fallback); canvas drawn by JS. |
| `flashcard` | `front`, `back` (both req). Click flips the card. |
| `math` | `tex` (req, KaTeX), `label` (optional), `definitions` (optional non-empty array of `{symbol, meaning, link?}`; `link` must start with `/` or `#`; renders as a collapsed `<details>` panel under the equation). |
| `compare` | Two side-by-side panels; `left`/`right`, each `{title?, tex (string or string array, req), note?}`; titles default "Original"/"Revised". Legacy widget from a previous comparison-style course; currently unused by content. |
| `stepper` | `label` (optional), `steps` (req, ≥ 2) of `{title?, tex?, text?, note?}` — each step needs `tex` or `text`. Server renders all steps; JS reveals one at a time (Prev/Next buttons, arrow keys, step counter). Automatically pulls in the `math` assets for KaTeX. |
| `interactive` | Client-evaluated curve with parameter sliders: `expression` (req — safe DSL: `+ - * / ^`, parens, variable `x`, `pi`, functions `exp log sqrt abs min max pow sin cos`), `domain = [min, max]` (req), optional `ydomain`, `parameters = [{name, label?, min, max, step?, initial?}]` (step defaults to range/100, initial to midpoint), `samples` (16–2000, default 240), `title`/`xlabel`/`ylabel`, `xslider` (bool; defaults on when the `readout` references `x` or `y` — renders an independent-variable slider so the readout has something to react to), `readout` (template with `{x}`, `{y}` — the expression value at the probe point — or `{paramname}` substitution; precision specs like `{x:0.2f}` allowed; validated server-side). The server parses the expression into a JSON AST; the client never sees source text to `eval`. |
| `acknowledge` | `statement` (req), `button_label` (default "I acknowledge"), `done_message` (default "Acknowledged"). Checkbox + button; the click persists to localStorage (`brillante.ack.v1`, keyed by widget id) and dispatches `brillante:ack`. A course with any `acknowledge` tag gets an automatic banner on its outline and lesson pages (`.ack-banner`, rendered hidden; `site.js` reveals it until one of the detected ids is acknowledged, reading the same `brillante.ack.v1` key) linking to the lesson containing the widget. Detection is automatic at load time via `scan_widget_tags_in_markdown` — no per-course config. |
| `resource-link` | Outbound card link for external materials: `title` (req), `url` (req, absolute http(s)), `source` (req, e.g. "MIT OpenCourseWare"), `description` (optional), `kind` (optional: `video\|pdf\|assignment\|exam\|web`, default `web`; affects icon/label). Opens in a new tab. |

`$...$`/`$$...$$` math in lesson markdown reuses the `math` widget's KaTeX
loader automatically — you only need a `math` *widget* when you want a label,
a definitions panel, or a named instance.

## Adding a new widget kind

Follow the existing pattern exactly — a widget is one Rust render function plus
a `.js`/`.css` pair, registered in one place:

1. **`src/widgets/<name>.rs`** — implement
   `pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError>`.
   - Read config through the accessors on `WidgetInstance`
     (`req_str`, `opt_str`, `req_f64`, `opt_f64`, `opt_usize`, `req_arr`,
     `opt_arr`, `opt_table`) and validate every key. Fail with
     `WidgetError::Invalid("clear message")` — the message is shown in the
     error card and logged; make it actionable ("'steps' must contain at least
     2 entries").
   - Build markup with leptos `view!`; static markup only, and give elements
     stable classes the JS can target. Wrap with
     `widget_shell("<name>", &w.id, serialize_config(w.config.as_ref()), inner)`.
     Never panic on bad config; return `Err`.
2. **`src/widgets/<name>.js` and `<name>.css`** — vanilla JS in an IIFE:
   on DOMContentLoaded, find `[data-widget-type="<name>"]` (or your classes),
   read the config JSON from `data-config`, mount behavior, and guard with
   `el.dataset.mounted` so double-init is a no-op (see `stepper.js`, the
   cleanest example). **No client framework, no `eval`/`new Function`.** CSS is
   scoped under the widget's classes.
3. **Register** in `WidgetRegistry::with_builtins()` in `src/widgets/mod.rs`:

   ```rust
   registry.register(WidgetDef {
       name: "my-widget",
       render: my_widget::render,
       script: include_str!("my-widget.js"),
       styles: include_str!("my-widget.css"),
   });
   ```

   Assets are `include_str!`d at compile time and served at
   `/widgets/<kind>/script.js` and `/style.css` — a rebuild is required after
   changes.
4. **Tests** — add the kind to the `registry_has_builtins` test in
   `src/widgets/mod.rs`; add colocated unit tests for parsing/validation
   (`#[cfg(test)] mod tests`, see `stepper.rs` and `math.rs` for the pattern);
   add integration tests in `tests/web.rs` if it affects page HTML.
5. If the widget renders TeX, emit `<div class="math-display" data-tex="...">`
   elements — `math.js` renders any such element, including inside your widget.

Good templates to copy: `flashcard.rs` (simplest), `stepper.rs` (validation +
tests), `interactive.rs` + `expr.rs` (server-side parsing, client AST eval).

When you add or change widget config keys, update this file and `README.md`.

## Platform invariants — do not break these

- **No author HTML is ever emitted.** The markdown pipeline renders semantic
  elements itself and all text is escaped. Never add a code path that passes
  author-controlled markup through to the page. This is the platform's
  security model.
- **Failures are contained.** Unknown widget type, missing config, invalid
  config → `error_shell` card; the lesson keeps rendering. Loader problems
  produce `Diagnostic`s, never crashes.
- **Diagnostics, not silence.** Warn on untitled lessons, duplicate slugs,
  invalid colors, bad frontmatter, dead links, unknown widget types. If you
  add a validation failure, emit a `Diagnostic` too.
- **SSR only.** Pages/widgets are leptos views stringified server-side
  (`src/render.rs`). Interactivity lives in the per-widget JS.
- **Sandboxed content serving.** `/content/{course}/{path}` blocks `.toml`
  files, dotfiles, and traversal; keep it that way.
- **State model.** `ContentStore` sits behind `Arc<RwLock<Arc<...>>>`;
  `POST /admin/reload` rebuilds it and swaps the `Arc`. The widget registry is
  fixed at startup. Lesson progress is client-side only
  (`localStorage`, key `brillante.progress.v1`) — no accounts, no DB.
- **Style.** Colocated `#[cfg(test)]` unit tests for parsing/validation logic;
  small pure functions; edition-2024 idioms; no comments unless explaining a
  non-obvious invariant.

## Working on the CRLB course (current project)

`tasks/plan.md` and `tasks/todo.md` describe an in-flight rebuild of
`content/crlb/` with per-task acceptance criteria, phases, and checkpoints —
**read them before touching content or widgets for this course.** Key points
that shape content decisions:

- The course is a self-contained first-principles derivation; notation is
  defined once in Lesson 1 and every later lesson links back (math widget
  `definitions` panels with intra-course anchors). No forward references to
  undefined symbols.
- Widget density is deliberate: quizzes after each major concept, math widgets
  with definitions panels for key equations, `interactive` widgets for
  quantitative trade-offs, `stepper` for long derivations, flashcards for
  symbol recall.
- Source material referenced by the plan (under `~/code/crlb_prove`) is
  **read-only reference**; the course must not cite the original paper's
  equation numbers.
- Every widget instance a lesson uses needs a matching `[widgets."<id>"]`
  entry in `content/crlb/config.toml` (~900 lines; organized in per-lesson
  sections). Adding a lesson means adding markdown **and** its config entries
  in the same change, then verifying via the reload/diagnostics loop.

## Gotchas

- Only `content/getting-started` is tracked in git; other directories under
  `content/` are ignored (personal courses stay local). New shared example
  content must go inside `getting-started` or the ignore rules must be
  deliberately widened.
- `tests/web.rs` is self-contained: it writes a fixture course into a temp dir
  and asserts only against that fixture. It must never reference course
  content shipped in the repo — extend the fixture when you add platform
  behavior that affects page HTML.
- Content edits are picked up by `POST /admin/reload`; `brillante.toml`
  registration changes also need a reload; Rust changes need a rebuild.
- Lesson order comes from frontmatter `order` first, then natural filename
  sort — a missing `order` (defaults 0.0) can silently reorder lessons.
- Widget `type` matching is case-insensitive but the `id` lookup is exact.
- Quiz `answer` defaults to index 0 when omitted — set it explicitly.
- KaTeX is loaded from a CDN at runtime; math falls back to raw TeX text
  offline (that is expected behavior, not a bug).
