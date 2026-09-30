# Optics Course — Implementation Plan

## Context

Build a new optics course for the Brillante platform from two sources in
`~/Documents/optics_course/` (read-only reference; never modify, never commit):

1. **MIT OCW 2.71/2.710 "Optics" (Spring 2009)** — full site mirror: 26-session calendar,
   lecture-notes PDFs (`mit2_71s09_lecNN.pdf`), video pages with YouTube IDs, transcripts
   (.srt/.vtt), assignments/quizzes/exams with solutions. Instructors Barbastathis, Sheppard,
   Oh. License **CC BY-NC-SA 4.0**. Canonical base URL:
   `https://ocw.mit.edu/courses/2-71-optics-spring-2009/` (per-resource:
   `/resources/<resource-slug>/`).
2. **LibreTexts PDF = OpenStax *University Physics III — Optics and Modern Physics***.
   Book page: `https://phys.libretexts.org/Bookshelves/University_Physics/University_Physics_(OpenStax)/University_Physics_III_-_Optics_and_Modern_Physics_(OpenStax)`.
   PDF download: `https://storage.downloads.libretexts.org/pdf/phys-4484/Full.pdf`. License
   **CC BY 4.0** (OpenStax). **Scope: chapters 1–4 only** (nature of light, geometric optics,
   interference, diffraction); modern-physics chapters are out of scope.

Videos are deliberately not downloaded: lessons link to MIT's OCW pages so traffic and credit
flow to MIT. Same principle applies to all downloadable materials (notes, assignments, exams):
**always link out**; copying *portions* into the course is acceptable when correctly cited.

## Decisions (resolved with user)

1. **Course shape**: synthesized topical lessons (not lesson-per-lecture) — merge adjacent MIT
   sessions, enrich with OpenStax ch 1–4. ~10 sections, 3–5 lessons each, ~38 lessons.
2. **Sections**: new reusable platform feature (course model has no grouping today).
3. **Acknowledgment**: new reusable `acknowledge` widget + automatic site-wide banner until
   acknowledged (localStorage, advisory only — platform has no server-side accounts).
4. **Source policy**: downloadables always link out; quoted/figure excerpts embedded with
   per-item citation.
5. **Repo**: standalone git repo at `/mnt/storage/code/optics-course`, registered in
   `brillante.toml` by absolute path. Submodule-ization of content is deferred (later task;
   config-path change only).
6. **Widgets**: all new widgets are course-agnostic. New: `acknowledge`, `resource-link`.
   Reuse existing `math` (definitions panels), `quiz`, `stepper`, `interactive`, `plot`,
   `slider`, `flashcard`.

## Platform tasks (Phase 1)

### Task 1 — Sections feature (reusable)
- `course.toml`: optional `[[sections]]` array of `{ title, slug?, description? }`; slug
  defaults to `slugify(title)`, duplicates get `-N` suffix + warning.
- Lesson frontmatter: optional `section = "<section-slug>"`. Unknown slug → warning, lesson
  renders ungrouped. Section with no lessons → warning. Ungrouped lessons render before
  declared sections (no heading).
- `Course` gains `sections: Vec<Section>`; `Lesson` gains `section: Option<String>`.
- Outline page (`course_page`): one `<h2 class="section-title">` + optional lede + its own
  `<ol class="lesson-list">` per section. Lesson numbering stays global; pager and progress
  counter stay flat (URLs unchanged — flat lesson slugs).
- Tests: `course.rs` unit tests (parsing, validation, diagnostics); `tests/web.rs` fixture
  asserts grouped outline HTML.

### Task 2 — `acknowledge` widget + site-wide banner (reusable)
- `src/widgets/acknowledge.rs` + `.js` + `.css` per the standard widget pattern. Config:
  `statement` (req), `button_label` (default "I acknowledge"), `done_message`
  (default "Acknowledged"). Renders statement + checkbox + button; JS persists to
  localStorage key `brillante.ack.v1` as `{ "<widget-id>": true }`, guarded by
  `el.dataset.mounted`, dispatches `brillante:ack` event.
- Auto-detection (no per-course config): `ContentStore` uses `scan_widget_tags`
  (`src/content/markdown.rs:205`) over each lesson body at load time to find `acknowledge`-type
  widget ids; store `ack_widget_ids` + `ack_lesson_slug` per course.
- `course_page` + `lesson_page` render a `<div class="ack-banner" hidden data-ack-ids=…
  data-ack-href=… data-ack-message=…>` when the course has any acknowledge widget.
  `site.js` (always loaded) removes `hidden` while none of the ids are acknowledged; listens
  for `brillante:ack`/`storage` to hide it. Banner links to the lesson containing the widget.
- Register in `WidgetRegistry::with_builtins()`; add to `registry_has_builtins` test; unit
  tests for config validation; `tests/web.rs` fixture asserts banner markup.

### Task 3 — `resource-link` widget (reusable)
- Card-style outbound link for external materials. Config: `title` (req), `url` (req),
  `source` (req, e.g. "MIT OpenCourseWare"), `description` (opt),
  `kind` (opt: `video|pdf|assignment|exam|web`, default `web`; affects icon/label).
  Pure static markup + no-op guarded JS. Used for MIT video/assignment/exam links and
  LibreTexts chapter links throughout the course.

### Task 4 — Docs + registry tests
- Update `AGENTS.md` widget table + `README.md`: `acknowledge`, `resource-link` kinds;
  `course.toml` `[[sections]]`; lesson frontmatter `section` key.

**Checkpoint 1**: `cargo build && cargo test && cargo clippy` clean; scratch course exercising
sections + acknowledge + resource-link renders correctly (then removed).

## Repo + course scaffold (Phase 2)

### Task 5 — Standalone course repo
- `git init /mnt/storage/code/optics-course` (initial branch `main`, empty tree first).
- `course.toml`: `title = "Optics"`, `slug = "optics"`, description (audience: undergrads
  with E&M + differential-equations background; derived from MIT 2.71 + OpenStax UP III with
  credit), `accent` (pick a distinct hex).
- Declare all sections; create `config.toml` skeleton with per-lesson commented sections;
  `images/` directory with a README noting naming + citation rules.
- Register in `/mnt/storage/code/brillante/brillante.toml`:
  `courses = ["content/getting-started", "/mnt/storage/code/optics-course"]`.
- Widget id namespace: `com.github.richbai90.optics.<lesson>.<name>`, unique per course.

### Task 6 — Lesson 1: "References, Licensing, and Citation Policy" (first lesson)
- Full citations of both sources (titles, authors, terms, licenses, URLs — MIT OCW base URL
  and LibreTexts book page + PDF link).
- License compatibility: MIT content is CC BY-NC-SA 4.0 → the course as a whole is released
  **CC BY-NC-SA 4.0**; OpenStax content remains CC BY 4.0 where used alone.
- Citation policy text (see conventions below) + `acknowledge` widget
  (`com.github.richbai90.optics.ack`) whose statement says the reader acknowledges the policy
  and that sources are cited throughout.
- "Further material" section with `resource-link` widgets: MIT video-lectures page, assignments
  page, exams page, LibreTexts book page.

**Checkpoint 2**: reload → `/admin/diagnostics` clean → lesson 1 renders, acknowledgment
persists across reloads, banner shows on other lessons until acknowledged.

## Content (Phase 3) — course skeleton

File naming `NN-slug.md`; `order` in frontmatter; `section` per table. Mapping to sources in
parentheses (MIT lecture numbers; OpenStax UP III chapters).

| Section | Lessons (mapped sources) |
|---|---|
| 1. Start Here | References & citation policy (Task 6) |
| 2. The Nature of Light | History of optics (lec 1; ch 1.1) · Light as wave & spectrum (lec 1; ch 1.1–1.2) · Light in matter: absorption, dispersion (lec 1–2; ch 1.3) |
| 3. Rays: Reflection & Refraction | Laws of reflection & refraction (lec 2; ch 1.3–1.4) · Prisms & dispersion (lec 2; ch 1.4) · Perfect focusing, aspheres, paraxial approx (lec 3; ch 2.1–2.2) · Ray transfer matrices (lec 3; supplementary notes) |
| 4. Lenses & Image Formation | Thin lenses, real/virtual images (lec 4; ch 2.2) · Thick & composite lenses (lec 5; ch 2.3) · The human eye & camera (lec 5; ch 2.4) · Mirrors (lec 7; ch 2.5) |
| 5. Apertures, Pupils & Instruments | Stops, pupils, windows, NA, FoV (lec 6) · Magnifiers & microscopes (lec 7; ch 2.7–2.8) · Telescopes (lec 8; ch 2.8) · Radiometry & photometry (lec 6 notes) |
| 6. Aberrations & Optical Design | Chromatic, spherical, coma (lec 8; ch 2.4.5) · Astigmatism, field curvature, distortion (lec 9) · GRIN optics & the design workflow (lec 9) |
| 7. Wave Optics Foundations | From rays to waves: Hamiltonian/eikonal (lec 11) · Wave equation, phasors, 3D waves (lec 12) · Maxwell's equations & EM waves (lec 13–14; ch 1.1–1.2) · Polarization & Poynting vector (lec 14; ch 1.7) |
| 8. Interference & Coherence | Huygens' principle & superposition (lec 15; ch 3.1–3.3) · Two-slit & multi-slit interference (lec 15; ch 3.4) · Thin-film interference (ch 3.5) · Michelson & interferometers (lec 15; ch 3.6) · Temporal & spatial coherence (lec 22) |
| 9. Diffraction & Fourier Optics | Fresnel diffraction (lec 15; ch 4.1) · Fraunhofer diffraction & Fourier transforms (lec 17; ch 4.2–4.3) · Gratings: amplitude & phase (lec 16; ch 4.4) · Spatial filtering & lens transforms (lec 18) · The 4f system, pupil masks, PSF, Talbot effect (lec 19–20) |
| 10. Imaging Systems & Resolution | ATF, OTF/MTF (lec 20, 22) · Coherent vs incoherent imaging (lec 22–23) · SBP, sampling (lec 20) · Resolution, defocus, depth of focus/field, deconvolution (lec 23, 25–26) · Wave plates & high-NA polarization effects (lec 26; ch 1.7) |

Per-section task = write N lessons **and** their `config.toml` entries in the same change,
then verify. Extract OpenStax ch 1–4 section list from the PDF first (pdftotext) and reconcile
the mapping above; figures extracted from sources go in `images/` with citation captions.

### Citation conventions (enforced per lesson)
- **Paraphrase by default.** Verbatim text only in markdown blockquotes with an explicit
  citation line + URL.
- Every lesson ends with a **Sources** footer listing what it drew on (MIT lecture N / OpenStax
  §x.y) with `resource-link` widgets or plain links to OCW canonical pages.
- Copied figures: `images/mit-lecN-<fig>.png` or `images/openstax-<ch>-<fig>.png`, caption ends
  with "(Source: … , CC BY-NC-SA 4.0 — <url>" / "CC BY 4.0 — <url>)".
- MIT videos/assignments/exams: `resource-link` pointing at the OCW page — never re-hosted.
- Every widget instance needs its `[widgets."<id>"]` entry in the same change (error cards are
  the failure signal otherwise).

### Widget density (follow the CRLB conventions)
2–4 quizzes per lesson; `math` widgets with definitions panels for key equations (definitions
link back to first-use anchors); `stepper` for long derivations (ray matrices, Fraunhofer);
`interactive`/`slider`+`plot` for quantitative trade-offs (Snell explorer, thin-lens imaging,
grating intensity, MTF vs defocus); `flashcard` decks for symbol recall.

**Checkpoint 3 (per section)**: reload → diagnostics clean → all lessons render with no
`widget-error` cards → links resolve → manual read-through (no undefined symbols).

## Final sweep (Phase 4)

- [ ] All widget ids unique across the course; no error shells anywhere.
- [ ] References lesson is lesson 1; acknowledgment flow works end-to-end (outline + lesson
      banner, persists across reloads).
- [ ] Citation audit: every verbatim block and every copied figure carries a citation; every
      lesson has a Sources footer; both source links present in lesson 1.
- [ ] `cargo test`, `cargo clippy` clean; diagnostics clean after full reload.
- [ ] Mirror the plan into `tasks/optics-plan.md` + `tasks/optics-todo.md` at implementation
      start (repo convention), update them as tasks complete.

## Risks

| Risk | Mitigation |
|---|---|
| License mixing (BY vs BY-NC-SA) | Course-level CC BY-NC-SA 4.0 notice in lesson 1; per-figure/source captions |
| Verbatim-copy drift | Paraphrase-by-default rule + citation audit in final sweep |
| OpenStax↔MIT topic misalignment | Reconcile against actual PDF TOC in Phase 3 start (pdftotext) |
| Widget config drift between `.md` and `config.toml` | Same-change rule + reload/diagnostics/page-check loop per task |
| `pdftotext`/`pdfimages` blocked by permission rules | User grants poppler tools when prompted during implementation |
| Sections feature scope creep | Flat URLs, single-level sections, no section-in-URL |

## Out of scope

- Reproducing MIT's syllabus/calendar/administrative pages, grading policies, student project
  galleries.
- Modern-physics chapters of OpenStax UP III (5+).
- Submodule restructuring of `content/` (deferred; config-only swap later).
- Re-hosting any MIT PDFs or video files.

## Open questions

- None blocking. Phase 3 start includes reconciling the section/lesson map against the actual
  LibreTexts PDF TOC.