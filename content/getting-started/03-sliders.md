---
title: Sliders and flashcards
description: Parameter exploration with slider widgets, and a two-sided flashcard for recall.
order: 3
---

The **slider** widget binds a range input to a config-driven value display.

<Widget type="slider" id="com.github.richbai90.slider_temperature" />

Sliders can also format their output with a `display` template, where
`{value}` is replaced by the current value:

<Widget type="slider" id="com.github.richbai90.slider_reaction" />

## Flip to recall

The **flashcard** widget shows a prompt on the front and reveals the answer on
click (or keyboard focus + Enter):

<Widget type="flashcard" id="com.github.richbai90.card_what_is_brillante" />

## Interactive, not scripted

None of this interactivity is inline HTML. Every widget's markup is produced
by the server, and each widget type ships its own small script and stylesheet
that the page loads only when that widget type appears in the lesson.
