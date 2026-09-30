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
| `/` | course catalog |
| `/course/{slug}` | course outline with progress |
| `/course/{slug}/{lesson}` | lesson page |
| `/widgets/{kind}/script.js`, `/widgets/{kind}/style.css` | per-kind assets |
| `/content/{course}/{path}` | files from the course directory (`.toml` and dotfiles blocked) |
| `/assets/site.css`, `/assets/site.js`, `/assets/favicon.svg` | platform assets |
| `/healthz`, `POST /admin/reload`, `/admin/diagnostics` | operations |

Lesson progress ("Mark complete") is stored client-side in `localStorage`.

## Development

```sh
cargo test      # unit + integration tests
cargo clippy
cargo run       # serve the example course
```

Architecture: `src/content/` (store, frontmatter, markdown→views pipeline),
`src/widgets/` (registry + built-ins), `src/web/` (axum router, pages, assets),
`src/render.rs` (leptos SSR helper). Pages and widgets are leptos views
rendered to HTML server-side; there is no client-side Rust/WASM.
