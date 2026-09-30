---
title: Welcome to Brillante
description: What this platform is, how lessons are written, and what happens to the HTML you were about to paste.
order: 1
---

Welcome! Brillante serves **courses** from registered directories of markdown files.
Each `.md` file in a registered directory is a *lesson*, and each lesson starts with
frontmatter that tells the server how to present it.

```markdown
---
title: My lesson
description: One line shown in the course outline.
order: 1
---
```

## What you can write

Standard markdown works the way you expect:

- **bold**, *italics*, `inline code`, and ~~strikethrough~~
- lists, numbered lists, and [links to other lessons](02-widgets.md)
- tables:

| Feature | Where it lives |
|:--- | ---: |
| Lesson metadata | frontmatter |
| Widget config | `config.toml` |
| Course metadata | `course.toml` |

> Quoted material renders as a blockquote. Everything you see here is plain
> markdown — no HTML anywhere in the source.

### Code fences keep their language

```rust
fn main() {
    println!("lessons are just files");
}
```

## What happens to raw HTML?

Nothing. Brillante renders markdown **only** — the single exception is the
special `<Widget />` tag, which the server resolves through its widget registry.

To prove the point, the source of this paragraph contains
`<script>alert('never rendered')</script>` and `<b>bold via HTML</b>`.
You will see neither a script tag nor a bold HTML tag below — both were
silently dropped by the renderer, because no author-supplied HTML is ever
emitted.
