---
title: Interactive widgets
description: The <Widget /> tag — type selects the renderer, id looks up the config in config.toml.
order: 2
---

A widget is embedded with a single self-closing tag:

```html
<Widget type="quiz" id="com.github.richbai90.quiz_basics" />
```

- **`type`** names the widget kind. The renderer looks it up in its registry —
  `quiz`, `slider`, `plot`, and `flashcard` are built in.
- **`id`** is the instance key. The server reads `config.toml` from the same
  directory as the course files and finds the matching `[widgets."<id>"]` table.

## A quiz

Here is the quiz configured under `com.github.richbai90.quiz_basics`:

<Widget type="quiz" id="com.github.richbai90.quiz_basics" />

Try a wrong answer first — then the right one. The explanation from
`config.toml` appears once you get it right.

## Widgets flow with the text

Widgets are block elements, but you can drop them mid-paragraph and the
renderer splits the paragraph around them. Here is a second quiz configured
under a different id, right in the middle of this sentence flow:

<Widget type="quiz" id="com.github.richbai90.quiz_check" />

And the paragraph continues after it, seamlessly.

## What if something is misconfigured?

Change the `id` below to something that is not in `config.toml` and you would
get a visible error card instead of a crash. As rendered, this lesson also
includes a widget whose type does not exist, so you can see exactly what your
readers would see:

<Widget type="nonexistent" id="com.github.richbai90.does_not_matter" />

The rest of the lesson still renders; the failure is contained to the widget.
