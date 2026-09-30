# TODO: Optics Course — Brillante build-out

Plan: `tasks/optics-plan.md`. Source material: `~/Documents/optics_course/` (read-only).

## Phase 1: Platform

## Task 1: Sections feature
**Description:** Optional `[[sections]]` array in `course.toml` (`{title, slug?, description?}`);
lesson frontmatter `section = "<slug>"`. Grouped outline rendering in `course_page`.
**Acceptance criteria:**
- [ ] Slug defaults to `slugify(title)`; duplicate slugs get `-N` suffix + warning
- [ ] Unknown section slug in lesson → warning, lesson renders ungrouped
- [ ] Declared section with no lessons → warning
- [ ] Ungrouped lessons render before declared sections, no heading
- [ ] One `<h2 class="section-title">` + optional lede + `<ol class="lesson-list">` per section
- [ ] Lesson numbering stays global; pager/progress/URLs unchanged
**Verification:**
- [ ] `cargo test` (course.rs unit tests + tests/web.rs grouped-outline fixture)
**Dependencies:** None
**Files:** `src/content/frontmatter.rs`, `src/content/course.rs`, `src/web/pages.rs`,
`src/web/site.css`, `tests/web.rs`
**Scope:** M

## Task 2: `acknowledge` widget + site-wide banner
**Description:** New widget kind (`statement` req, `button_label`/`done_message` opt).
JS persists acknowledgment to localStorage `brillante.ack.v1` as `{id: true}`, dispatches
`brillante:ack`. ContentStore auto-detects acknowledge ids via `scan_widget_tags` at load
time; pages render `.ack-banner` when present; `site.js` shows/hides it.
**Acceptance criteria:**
- [ ] Missing/invalid config → error card (contained)
- [ ] JS guarded by `dataset.mounted`; double-init is a no-op
- [ ] Banner hidden once any listed id acknowledged; reappears if storage cleared
- [ ] Banner links to lesson containing the widget
**Verification:**
- [ ] `cargo test` (widget unit tests + tests/web.rs banner markup)
- [ ] Manual: banner flow across pages/reloads
**Dependencies:** None
**Files:** `src/widgets/acknowledge.rs/js/css`, `src/widgets/mod.rs`, `src/content/mod.rs`,
`src/content/course.rs`, `src/web/pages.rs`, `src/web/site.js`, `src/web/site.css`,
`tests/web.rs`
**Scope:** L

## Task 3: `resource-link` widget
**Description:** Card-style outbound link: `title` (req), `url` (req), `source` (req),
`description` (opt), `kind` (opt: video|pdf|assignment|exam|web, default web).
**Acceptance criteria:**
- [ ] Invalid kind → clear WidgetError
- [ ] Static markup; no-op guarded JS; external link opens in new tab
**Verification:**
- [ ] `cargo test` (validation unit tests + registry test)
**Dependencies:** None
**Files:** `src/widgets/resource_link.rs/js/css`, `src/widgets/mod.rs`
**Scope:** S

## Task 4: Docs + registry tests
**Description:** AGENTS.md widget table + README: `acknowledge`, `resource-link`,
`[[sections]]`, frontmatter `section`.
**Acceptance criteria:**
- [ ] Docs list config keys and section behavior accurately
**Verification:**
- [ ] `registry_has_builtins` covers new kinds
**Dependencies:** Tasks 1–3
**Files:** `AGENTS.md`, `README.md`, `src/widgets/mod.rs` (test)
**Scope:** S

## Checkpoint 1: Platform
- [ ] `cargo build && cargo test && cargo clippy` clean
- [ ] Scratch course exercising sections + acknowledge + resource-link renders correctly
- [ ] Scratch course removed

## Phase 2: Repo + scaffold

## Task 5: Standalone course repo
**Description:** `git init /mnt/storage/code/optics-course` on `main`; `course.toml`
(title/slug/description/accent), sections declared, `config.toml` skeleton, `images/README`;
register in `brillante.toml`.
**Acceptance criteria:**
- [ ] Course loads on the server home page
- [ ] Widget id namespace `com.github.richbai90.optics.<lesson>.<name>`
**Verification:**
- [ ] Server restart → course listed; diagnostics clean
**Dependencies:** Checkpoint 1
**Files:** `/mnt/storage/code/optics-course/*`, `brillante.toml`
**Scope:** S

## Task 6: Lesson 1 — References, Licensing, and Citation Policy
**Description:** Full citations of MIT OCW 2.71 (CC BY-NC-SA 4.0) + OpenStax UP III (CC BY 4.0);
license-compatibility statement; citation policy; `acknowledge` widget
(`com.github.richbai90.optics.ack`); "Further material" resource-links.
**Acceptance criteria:**
- [ ] Both sources fully cited with URLs and licenses
- [ ] Acknowledge widget present and persists
- [ ] resource-links to MIT video-lectures/assignments/exams + LibreTexts book page
**Verification:**
- [ ] reload → diagnostics clean → lesson renders without error cards
- [ ] Banner shows on other lessons until acknowledged
**Dependencies:** Task 5
**Files:** `/mnt/storage/code/optics-course/01-references.md`, `config.toml`
**Scope:** S

## Checkpoint 2: Scaffold done
- [ ] reload → `/admin/diagnostics` clean
- [ ] Lesson 1 renders; acknowledgment persists across reloads
- [ ] Banner flow works end-to-end

## Phase 3: Content (sections 2–10)
Per-section task = write lessons **and** config entries in the same change, then verify:
reload → diagnostics clean → pages render with no `widget-error` cards → links resolve.

## Task 7: Phase 3 prep — OpenStax TOC extraction + map reconciliation
**Description:** Extract LibreTexts PDF TOC (pdftotext), verify ch 1–4 section numbering,
reconcile the plan's lesson table.
**Dependencies:** Checkpoint 2
**Scope:** S

**Status: done.** LibreTexts PDF numbering: ch 1 = 1.2 propagation, 1.3 reflection,
1.4 refraction, 1.5 TIR, 1.6 dispersion, 1.7 Huygens, 1.8 polarization; ch 2 = 2.2 plane
mirrors, 2.3 spherical mirrors, 2.4 refraction images, 2.5 thin lenses, 2.6 eye, 2.7 camera,
2.8 magnifier, 2.9 microscopes/telescopes. Lessons 05–08 citations already match this
numbering; the plan's mapping table (§2.2 thin lenses etc.) is stale — trust lesson
citations. OpenStax has no thick-lens/principal-plane section (MIT lec 5 only).

## Task 8: Section 2 — The Nature of Light (3 lessons)
**Files:** `02-history.md`, `03-wave-spectrum.md`, `04-light-in-matter.md`, `config.toml`
**Dependencies:** Task 7

**Status: done** (02–04 written with configs in a prior session).

## Task 9: Section 3 — Rays: Reflection & Refraction (4 lessons)
**Files:** `05-reflection-refraction.md`, `06-prisms.md`, `07-focusing-paraxial.md`,
`08-ray-matrices.md`, `config.toml`
**Dependencies:** Task 8

**Status: done.** Lessons 05–08 were written in a prior session but their 34 config
entries were missing (error cards on every page); configs written and forward links
aligned to the planned lesson filenames. Verified: diagnostics clean (minus expected
empty-section warnings), zero error cards, dead links only to future lessons.

## Task 10: Section 4 — Lenses & Image Formation (4 lessons)
**Files:** `09-thin-lenses.md`, `10-thick-composite.md`, `11-eye-camera.md`, `12-mirrors.md`
**Dependencies:** Task 9

**Status: done.** All four lessons + 37 config entries written and verified (reload →
6 expected empty-section warnings only → 0 error cards on all 12 lessons; outline
groups section 4 correctly). Dead links remaining on these pages point only at lessons
from Tasks 11–16 (15-telescopes, 17-chromatic-spherical-coma, 20-rays-to-waves,
24-huygens-superposition, 33-fourf-system, 37-resolution-defocus).

## Task 11: Section 5 — Apertures, Pupils & Instruments (4 lessons)
**Files:** `13-stops-pupils.md`, `14-magnifiers-microscopes.md`, `15-telescopes.md`,
`16-radiometry.md`
**Dependencies:** Task 10

## Task 12: Section 6 — Aberrations & Optical Design (3 lessons)
**Files:** `17-chromatic-spherical-coma.md`, `18-astig-field-distortion.md`, `19-grin-design.md`
**Dependencies:** Task 11

## Task 13: Section 7 — Wave Optics Foundations (4 lessons)
**Files:** `20-rays-to-waves.md`, `21-wave-equation-phasors.md`, `22-maxwell-em-waves.md`,
`23-polarization-poynting.md`
**Dependencies:** Task 12

## Task 14: Section 8 — Interference & Coherence (5 lessons)
**Files:** `24-huygens-superposition.md`, `25-slit-interference.md`, `26-thin-films.md`,
`27-interferometers.md`, `28-coherence.md`
**Dependencies:** Task 13

## Task 15: Section 9 — Diffraction & Fourier Optics (5 lessons)
**Files:** `29-fresnel.md`, `30-fraunhofer-fourier.md`, `31-gratings.md`,
`32-spatial-filtering.md`, `33-fourf-system.md`
**Dependencies:** Task 14

## Task 16: Section 10 — Imaging Systems & Resolution (5 lessons)
**Files:** `34-atf-otf-mtf.md`, `35-coherent-vs-incoherent.md`, `36-sbp-sampling.md`,
`37-resolution-defocus.md`, `38-wave-plates-high-na.md`
**Dependencies:** Task 15

## Phase 4: Final sweep

## Task 17: Citation + integration audit
**Acceptance criteria:** unique widget ids; no error shells; references lesson is lesson 1;
ack flow end-to-end; every verbatim block and copied figure cited; every lesson has Sources
footer; `cargo test && cargo clippy` clean; diagnostics clean after full reload; update
`tasks/optics-plan.md`/`todo.md` checkboxes.
**Dependencies:** Tasks 7–16
**Scope:** M