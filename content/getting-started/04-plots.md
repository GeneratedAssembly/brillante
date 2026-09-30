---
title: Plots and course links
description: Data-driven plot widgets, and linking between lessons by file name.
order: 4
---

The **plot** widget draws its series from `config.toml` — points, colors,
names, and axis labels are all data, not markup:

<Widget type="plot" id="com.github.richbai90.plot_growth" />

## Linking lessons together

Relative markdown links that point at a sibling `.md` file are rewritten into
lesson URLs automatically. This link to the first lesson works:

[Back to *Welcome to Brillante*](01-welcome.md)

Links may also carry fragments: [the quiz section of the widget lesson](02-widgets.md#a-quiz).

## Images from the course directory

Relative image sources are served from the registered course directory:

![The Brillante mark](images/logo.svg)

## Wrapping up

You have seen the whole content model now:

1. Register directories in `brillante.toml` (or `--course`).
2. Write lessons in markdown with frontmatter.
3. Configure widget instances in `config.toml` next to the course files.
4. Embed widgets with `<Widget type="..." id="..." />` — and nothing else
   HTML-shaped ever reaches the page.

*Ready to write your own course? Copy the `content/getting-started` directory
and start editing.*
