# Implementation Plan: CRLB Course — First-Principles Rebuild

## Overview

Rebuild the `crlb` course in `content/crlb/` as a **self-contained derivation from first
principles**, removing the dependence on the original paper's notation. The current course is
structured as a running comparison ("original Eq. (N) vs revised Eq. (M)"), which forces the
learner to hold two numbering schemes in their head. The new course derives everything from
scratch: probability → score → Fisher information → the event-sensor forward model → the
likelihood → the spectral FIM, with one consistent notation throughout.

Source material (read-only reference; do not modify):
- `~/code/crlb_prove/CRLB.pdf` — original 4D derivation (for authorial fidelity only; the new
  course must **not** cite its equation numbers)
- `~/code/crlb_prove/derivation/main.tex` — revised derivation; the mathematical ground truth
  for the new course (equations (1)–(28))
- `~/code/crlb_prove/lean/CRLB.lean` — machine-verified theorems; each course equation that has
  a Lean proof gets a "machine-verified" badge linking the theorem name
- `~/code/crlb_prove/CRLB and Hyperspectral Model Critique.txt` — narrative motivation for the
  modeling choices (sum-before-log, point process, two noise domains, scaling retraction)

## Design Decisions

1. **Single notation, defined once.** All symbols come from one glossary lesson (Lesson 1).
   Every lesson uses only symbols defined in Lesson 1 or earlier. No "Eq. (N) of the original
   paper" references anywhere — the original paper's role is confined to optional historical
   asides in the final lesson.
2. **Collapsed "Definitions" sections under every equation.** The `math` widget gains an
   optional `definitions` config key: a list of `{symbol, meaning, link}` entries rendered as a
   collapsed-by-default `<details>` panel under the equation. `link` is an intra-course anchor
   (`/crlb/lesson-slug#anchor`) pointing to where the symbol was first defined. This requires
   extending `src/widgets/math.rs` + `math.css` (backwards-compatible: absent key → current
   rendering).
3. **Widget density.** "More is better than less": every lesson carries
   - `math` widgets with definitions panels (all key equations),
   - `quiz` checks after each major concept (2–4 per lesson),
   - `slider` + `plot` for every quantitative trade-off,
   - `flashcard` for symbol recall,
   - a new **`stepper`** widget: a multi-step worked derivation, one line of math per step,
     revealed sequentially with a "next" button — for proofs too long for a single `math` block.
4. **Interactive plots with adjustable controls.** The `plot` widget already renders static
   series. Rather than extend the platform's plot widget with live re-computation (large
   platform change), each interactive exploration pairs a `slider` (parameter) with a `plot`
   whose caption explains what to look for, plus new **`interactive`** widget type: config
   declares a function family (`kind = "function"` with `expression`, `parameters`, ranges)
   evaluated client-side so one widget = curve + sliders + live readout. This is a new widget
   (`src/widgets/interactive.rs/js/css`) with a small safe expression evaluator (no eval;
   whitelist of operations parsed from config).
5. **Course flow (dependency order):**
   1. *Notation and Probability Toolkit* — random variables, expectation, parametrization
      `p(x;θ)`, likelihood, log-likelihood. Flashcards + quizzes.
   2. *Estimation and the Score* — unbiased estimators, score function, zero-mean property
      (derived via interactive differentiation-under-integral animation).
   3. *Fisher Information and the CRLB* — both FIM forms, Cauchy–Schwarz → bound; curvature
      intuition (interactive: adjust distribution width, watch I(θ) and the bound move).
   4. *The Poisson Count Model* — PMF, score, I(Λ)=Δt/Λ; slider explores the Δt/Λ trade-off.
   5. *A Sensor That Sees Change* — event-sensor physics: log front-end, thresholds,
      trigger conditions (with the signed-threshold fix presented as *the correct design*, not
      a correction of a numbered equation), material vs fixed-pixel derivative.
   6. *Sum Before Log* — hyperspectral encoding, background B_j, contrast q_j, the B=0
      degeneracy, sensitivity engine (Eq. 10). Interactive: observability–dilution curve.
   7. *Events as a Point Process* — marked point process likelihood, survival integral,
      binned-vs-continuous equivalence (stepper widget walks Eq. 16→21).
   8. *The Spectral Fisher Matrix* — chain rule with polarity, spectral block (Eq. 24),
      kernel/gauge identifiability, Schur complement, reconstruction (Eq. 25).
   9. *Scaling with Light* — three noise domains; interactive plot comparing the retracted
      CRLB∝N claim with the conditional model; historical aside tying back to the original
      paper (the *only* place the original is discussed).
   10. *Machine-Verified* — Lean layer map; theorem ↔ equation table; scope and limits.
6. **Vertical slicing.** Each lesson task delivers complete, testable content (markdown +
   config.toml widget entries + any new widget code needed by it), verifiable by loading the
   server and reading the rendered page. Platform tasks (math definitions panel, stepper,
   interactive) are separate, earlier tasks because lessons depend on them.

## Architecture Decisions

- **Extend, don't fork, existing widgets.** New features ride the existing
  `WidgetDef`/`widget_shell`/config.toml machinery.
- **New widget: `stepper`** (server renders all steps; JS reveals one at a time). Config:
  `steps = [{title, tex or text, note}]`.
- **New widget: `interactive`** — client-evaluated curve with bound sliders. Config:
  `xlabel`, `ylabel`, `expression` (in a small safe DSL over variable `x` and named
  parameters), `domain = [min, max]`, `parameters = [{name, min, max, step, initial, label}]`,
  optional `readout` template. Evaluator: recursive-descent parser over `+ - * / ^`, parens,
  numbers, parameter names, `exp log sqrt abs min max` — parsed once server-side, serialized
  as an AST in the config JSON for the client (no `eval`).
- **Definitions panel on `math` widget**: optional `definitions = [{symbol, meaning, link?}]`
  rendered as `<details class="math-defs">`, collapsed by default, links are plain anchors.
- **Markdown `$$…$$` display equations stay** for flowing derivations (already rendered);
  `math` widgets are reserved for equations that need the definitions panel, a Lean badge, or
  an equation number.

## Task List

### Phase 1: Platform (widget capabilities)

- [ ] Task 1: Add `definitions` panel to the `math` widget (S)
- [ ] Task 2: New `stepper` widget (M)
- [ ] Task 3: New `interactive` widget with safe client-side expression evaluation (L —
      break into 3a parser+AST serde, 3b render+js, 3c tests if it grows past ~5 files)

### Checkpoint: Platform
- [ ] `cargo build` clean, `cargo test` passes
- [ ] Server renders a scratch lesson exercising math-defs, stepper, interactive without error
- [ ] Remove scratch lesson

### Phase 2: Foundation lessons (first principles)

- [ ] Task 4: Lesson 1 — Notation and Probability Toolkit (S)
- [ ] Task 5: Lesson 2 — Estimation and the Score (S)
- [ ] Task 6: Lesson 3 — Fisher Information and the CRLB (M)
- [ ] Task 7: Lesson 4 — The Poisson Count Model (S)

### Checkpoint: Foundation
- [ ] Lessons 1–4 render, quizzes answerable, plots/sliders functional
- [ ] Every equation has a definitions panel; every symbol's link resolves
- [ ] Manual read-through: no forward references to undefined symbols

### Phase 3: The event sensor (core features)

- [ ] Task 8: Lesson 5 — A Sensor That Sees Change (M)
- [ ] Task 9: Lesson 6 — Sum Before Log (M)
- [ ] Task 10: Lesson 7 — Events as a Point Process (M)

### Checkpoint: Core
- [ ] The full derivation chain Lesson 1 → 7 is internally consistent
- [ ] Interactive widgets match the equations (spot-check numerics)
- [ ] End-to-end read: a learner can go from "what is expectation" to q_j and r_jp(t;θ)

### Phase 4: Results and verification

- [ ] Task 11: Lesson 8 — The Spectral Fisher Matrix (M)
- [ ] Task 12: Lesson 9 — Scaling with Light (S)
- [ ] Task 13: Lesson 10 — Machine-Verified in Lean 4 (S)

### Checkpoint: Complete
- [ ] All acceptance criteria met; old comparison-style content fully replaced
- [ ] `cargo test` passes; all widget configs valid (no error shells on any page)
- [ ] Ready for review

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Interactive expression evaluator scope creep | Med | Hard-limit DSL surface; AST parsed server-side; reject unknown tokens at config load |
| Definitions-panel links rot when lessons reorder | Low | Links point to stable lesson slugs + heading anchors, not lesson numbers |
| Losing the original-paper context learners may want | Low | Lesson 9 historical aside + Lesson 10 mapping table cover it deliberately |
| Widget config drift between lesson md and config.toml | Med | Manual reload + page-load check per task; error shells surface missing configs loudly |

## Open Questions

- None blocking. (If the `interactive` widget's safe-evaluator proves too heavy, fallback is
  precomputed parameter-sweep plots + sliders, which needs no new platform code.)
