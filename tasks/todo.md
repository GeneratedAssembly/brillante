# TODO: CRLB Course — First-Principles Rebuild

Plan: `tasks/plan.md`. Source material: `~/code/crlb_prove` (read-only).

## Phase 1: Platform

## Task 1: Math widget definitions panel
**Description:** Add optional `definitions` key to `math` widget config: array of
`{symbol, meaning, link?}`. Render collapsed-by-default `<details>` under the equation.
**Acceptance criteria:**
- [ ] `definitions` absent → rendering identical to today
- [ ] present → collapsed panel, expandable, symbol in mono, link renders as anchor
- [ ] invalid entries produce a clear `WidgetError::Invalid`
**Verification:**
- [ ] `cargo test`
- [ ] Manual: scratch lesson with/without definitions
**Dependencies:** None
**Files:** `src/widgets/math.rs`, `src/widgets/math.css`
**Scope:** S

## Task 2: Stepper widget
**Description:** Multi-step worked derivation: `steps = [{title, tex?, text?, note?}]`;
server renders all steps, JS shows one at a time with Prev/Next and step counter.
**Acceptance criteria:**
- [ ] ≥2 steps required; each step has tex or text
- [ ] keyboard accessible (buttons, arrow keys)
- [ ] KaTeX renders step tex
**Verification:**
- [ ] `cargo test`; manual click-through in browser
**Dependencies:** None
**Files:** `src/widgets/stepper.rs/js/css`, `src/widgets/mod.rs` (registry)
**Scope:** M

## Task 3: Interactive widget (curve + parameter sliders)
**Description:** Client-evaluated function plot with bound parameter sliders. Server parses
`expression` into a safe AST (no eval); client evaluates AST per slider move and redraws.
Config: `expression`, `domain`, `parameters`, `xlabel`, `ylabel`, `readout?`.
**Acceptance criteria:**
- [ ] Server-side parser accepts `+ - * / ^`, parens, numbers, `x`, params, `exp log sqrt abs`
- [ ] Unknown tokens rejected at config load with clear error
- [ ] Slider drag redraws curve without page reload; readout template `{name}` substitution
- [ ] No `eval`/`new Function` in shipped JS
**Verification:**
- [ ] `cargo test` (parser unit tests)
- [ ] Manual: dither curve B/(B+af)² vs sliders B, f
**Dependencies:** None
**Files:** `src/widgets/interactive.rs/js/css`, `src/widgets/mod.rs`
**Scope:** L (split 3a parser+tests, 3b render+js+css, 3c registry+docs if >5 files)

## Checkpoint: Platform
- [ ] `cargo build` && `cargo test` clean
- [ ] Scratch lesson exercises all three additions without error shells
- [ ] Scratch lesson removed

## Phase 2: Foundation lessons

## Task 4: Lesson 1 — Notation and Probability Toolkit
**Description:** Random variables, expectation, parametric distribution `p(x;θ)` vs
conditioning, likelihood vs probability, log-likelihood. Glossary flashcards.
**Acceptance criteria:**
- [ ] All notation defined here is used consistently later; stable heading anchors
- [ ] ≥2 quizzes, ≥3 flashcards, ≥1 math widget with definitions
**Verification:** server render + manual read
**Dependencies:** Task 1
**Files:** `content/crlb/01-notation.md`, `content/crlb/config.toml`
**Scope:** S

## Task 5: Lesson 2 — Estimation and the Score
**Description:** Unbiased estimator; score function; zero-mean property (derived, not cited).
Stepper walks the interchange-of-differentiation argument.
**Acceptance criteria:**
- [ ] Score and zero-mean shown as math widgets with definitions panels
- [ ] Stepper derivation of E[V]=0; ≥2 quizzes
**Verification:** server render + manual read
**Dependencies:** Task 2, 4
**Files:** `content/crlb/02-score.md`, `content/crlb/config.toml`
**Scope:** S

## Task 6: Lesson 3 — Fisher Information and the CRLB
**Description:** Both FIM forms + equivalence; Cauchy–Schwarz to the matrix bound; curvature
intuition. Interactive: distribution width vs I(θ) and CRLB floor.
**Acceptance criteria:**
- [ ] FIM, CRLB widgets with definitions; interactive width↔information widget
- [ ] ≥2 quizzes, ≥2 flashcards
**Verification:** server render; numerics spot-check
**Dependencies:** Task 3, 5
**Files:** `content/crlb/03-fisher.md`, `content/crlb/config.toml`
**Scope:** M

## Task 7: Lesson 4 — The Poisson Count Model
**Description:** PMF → log-likelihood → score → I(Λ)=Δt/Λ, derived step by step; slider
explores Δt/Λ trade-off; note that this algebra is exact and survives unchanged.
**Acceptance criteria:**
- [ ] All four steps as math widgets with definitions; ≥2 quizzes
- [ ] Slider demonstrates information falling with rate
**Verification:** server render
**Dependencies:** Task 6
**Files:** `content/crlb/04-poisson.md`, `content/crlb/config.toml`
**Scope:** S

## Checkpoint: Foundation
- [ ] Lessons 1–4 render clean; no forward references to undefined symbols
- [ ] All definition links resolve
- [ ] Review before proceeding

## Phase 3: The event sensor

## Task 8: Lesson 5 — A Sensor That Sees Change
**Description:** Event-sensor physics: log front-end, contrast threshold, signed trigger
conditions with reference reset (presented as the correct design), material vs fixed-pixel
derivative. Animation: pixel accumulating log-intensity, crossing ±C.
**Acceptance criteria:**
- [ ] Trigger condition and q_j material-derivative split as math widgets with definitions
- [ ] ≥1 animation/interactive (event stream vs moving source), ≥2 quizzes
**Verification:** server render
**Dependencies:** Task 4 (2, 3)
**Files:** `content/crlb/05-sensor.md`, `content/crlb/config.toml`
**Scope:** M

## Task 9: Lesson 6 — Sum Before Log
**Description:** Hyperspectral encoding H, s, h_j; background B_j; gated intensity
I_j = B_j + a f_j; contrast q_j; the B=0 degeneracy; sensitivity engine ∇_s q_j.
**Acceptance criteria:**
- [ ] Eqs (5)–(10) as math widgets with definitions; Lean badges where proven
- [ ] Interactive observability–dilution curve with live sliders; degeneracy shown numerically
- [ ] ≥2 quizzes, ≥2 flashcards
**Verification:** server render; numerics match Eq. (10)
**Dependencies:** Task 8
**Files:** `content/crlb/06-sum-before-log.md`, `content/crlb/config.toml`
**Scope:** M

## Task 10: Lesson 7 — Events as a Point Process
**Description:** Marked point process likelihood, survival integral, score, FIM (Eq. 15);
binned derivation Eqs (16)–(21) via stepper; exactness for piecewise-constant rates.
**Acceptance criteria:**
- [ ] Eqs (12)–(21) as math widgets with definitions; survival integral interactive
- [ ] Stepper walks binned score→FIM; ≥2 quizzes
**Verification:** server render
**Dependencies:** Task 9
**Files:** `content/crlb/07-point-process.md`, `content/crlb/config.toml`
**Scope:** M

## Checkpoint: Core
- [ ] Derivation chain 1→7 internally consistent
- [ ] Interactive numerics spot-checked against equations
- [ ] End-to-end read-through complete

## Phase 4: Results and verification

## Task 11: Lesson 8 — The Spectral Fisher Matrix
**Description:** Rate chain rule with polarity (Eq. 23), spectral block (Eq. 24), three
regimes, kernel/gauge caveats, Schur complement (Eqs. 27–28), reconstruction (Eq. 25).
**Acceptance criteria:**
- [ ] All equations as math widgets with definitions + Lean badges
- [ ] Interactive: spectral entry vs B_j with sliders; ≥2 quizzes
**Verification:** server render
**Dependencies:** Task 10
**Files:** `content/crlb/08-spectral-fim.md`, `content/crlb/config.toml`
**Scope:** M

## Task 12: Lesson 9 — Scaling with Light
**Description:** Three noise domains; photon/circuit/optical; conditional scaling; the CRLB∝N
claim and why it fails (historical aside — the only place the original paper is discussed).
**Acceptance criteria:**
- [ ] Interactive dual-curve scaling plot with adjustable N; ≥2 quizzes
**Verification:** server render
**Dependencies:** Task 11
**Files:** `content/crlb/09-scaling.md`, `content/crlb/config.toml`
**Scope:** S

## Task 13: Lesson 10 — Machine-Verified in Lean 4
**Description:** Theorem ↔ equation map table, four proof layers, scope (what is not proven),
links to `~/code/crlb_prove/lean/CRLB.lean`.
**Acceptance criteria:**
- [ ] Table complete against `CRLB.lean` theorem list; ≥1 quiz
**Verification:** server render; theorem names match file
**Dependencies:** Task 11
**Files:** `content/crlb/10-lean.md`, `content/crlb/config.toml`
**Scope:** S

## Checkpoint: Complete
- [ ] All acceptance criteria met; old comparison-style content fully replaced
- [ ] `cargo test` passes; no widget error shells anywhere in the course
- [ ] Old lesson files (`01-foundation.md` … `07-lean.md`) removed after replacement
- [ ] Ready for review
