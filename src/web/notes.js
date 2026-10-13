(() => {
  "use strict";

  document.addEventListener("DOMContentLoaded", () => {
    const aside = document.getElementById("notebook");
    if (!aside || aside.dataset.mounted) return;
    aside.dataset.mounted = "1";

    const toggle = document.getElementById("notebook-toggle");
    const input = aside.querySelector("[data-notebook-input]");
    const anchorField = aside.querySelector("[data-notebook-anchor]");
    const hereField = aside.querySelector("[data-notebook-here]");
    const previewBtn = aside.querySelector("[data-notebook-preview]");
    const previewOut = aside.querySelector("[data-notebook-preview-out]");
    const pop = aside.querySelector("[data-notebook-complete]");
    if (!input || !anchorField || !hereField || !pop) return;

    const lessons = readLessons();
    const headings = readHeadings();

    function readLessons() {
      const node = aside.querySelector("[data-notebook-lessons]");
      if (!node) return [];
      try {
        const parsed = JSON.parse(node.textContent);
        return Array.isArray(parsed) ? parsed : [];
      } catch {
        return [];
      }
    }

    function readHeadings() {
      const out = [];
      document.querySelectorAll(".lesson-body h2[id], .lesson-body h3[id]").forEach((h) => {
        const text = h.textContent.trim();
        if (text) out.push({ id: h.id, text });
      });
      return out;
    }

    function setOpen(open) {
      aside.classList.toggle("open", open);
      if (toggle) toggle.setAttribute("aria-expanded", String(open));
    }

    document.body.classList.add("notebook-drawer");
    if (toggle) {
      toggle.hidden = false;
      toggle.addEventListener("click", () => setOpen(!aside.classList.contains("open")));
    }
    const closeBtn = aside.querySelector("[data-notebook-close]");
    if (closeBtn) closeBtn.addEventListener("click", () => setOpen(false));

    if (previewBtn && previewOut && window.fetch) {
      previewBtn.addEventListener("click", () => {
        const course = document.body.dataset.course;
        const lesson = document.body.dataset.lesson;
        if (!course || !lesson) return;
        previewBtn.disabled = true;
        fetch("/api/notes/preview", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            course,
            lesson,
            body: input.value,
            anchor: anchorField.value,
            here_title: hereField.value,
          }),
        })
          .then((r) => (r.ok ? r.text() : Promise.reject(new Error(String(r.status)))))
          .then((html) => {
            previewOut.innerHTML = html;
            if (window.brillanteMath) window.brillanteMath(previewOut);
            previewOut.hidden = false;
          })
          .catch(() => {
            previewOut.textContent = "Preview is unavailable right now.";
            previewOut.hidden = false;
          })
          .finally(() => {
            previewBtn.disabled = false;
          });
      });
    }

    let items = [];
    let active = -1;

    function close() {
      pop.hidden = true;
      items = [];
      active = -1;
    }

    function markActive() {
      pop.querySelectorAll("[data-note-item]").forEach((b, i) => {
        if (i === active) b.setAttribute("data-active", "");
        else b.removeAttribute("data-active");
      });
    }

    function open(t, list) {
      items = list;
      active = list.length ? 0 : -1;
      pop.replaceChildren();
      list.forEach((it, i) => {
        const b = document.createElement("button");
        b.type = "button";
        b.setAttribute("data-note-item", String(i));
        b.textContent = it.label;
        if (it.kind) {
          const s = document.createElement("span");
          s.className = "note-item-kind";
          s.textContent = it.kind;
          b.append(s);
        }
        b.addEventListener("mousedown", (e) => {
          e.preventDefault();
          choose(i);
        });
        pop.append(b);
      });
      markActive();
      pop.hidden = false;
    }

    function tokenBefore() {
      const pos = input.selectionStart;
      const upto = input.value.slice(0, pos);
      if (upto.endsWith("@")) {
        const prev = upto.slice(0, -1).slice(-1);
        if (prev && /[A-Za-z0-9_]/.test(prev)) return null;
        return { mode: "heading", query: "", start: pos - 1 };
      }
      const m = /\[\[([^\[\]]*)$/.exec(upto);
      if (m) return { mode: "lesson", query: m[1], start: pos - m[0].length };
      return null;
    }

    function choose(i) {
      const it = items[i];
      const t = tokenBefore();
      if (!it || !t) {
        close();
        return;
      }
      const pos = input.selectionStart;
      const before = input.value.slice(0, t.start);
      const after = input.value.slice(pos);
      let insert;
      if (t.mode === "heading") {
        anchorField.value = it.id;
        hereField.value = it.text;
        insert = "@here ";
      } else {
        insert = "[[" + it.slug + "]] ";
      }
      input.value = before + insert + after;
      const caret = (before + insert).length;
      input.setSelectionRange(caret, caret);
      input.focus();
      close();
    }

    input.addEventListener("input", () => {
      const t = tokenBefore();
      if (!t) {
        close();
        return;
      }
      const q = t.query.toLowerCase();
      if (t.mode === "heading") {
        open(
          t,
          headings
            .filter((h) => !q || h.text.toLowerCase().includes(q) || h.id.includes(q))
            .map((h) => ({ id: h.id, text: h.text, label: "\u00A7 " + h.text, kind: "#" + h.id }))
        );
      } else {
        open(
          t,
          lessons
            .filter((l) => !q || l.slug.includes(q) || l.title.toLowerCase().includes(q))
            .map((l) => ({ slug: l.slug, label: l.title, kind: l.slug }))
        );
      }
    });

    input.addEventListener("keydown", (e) => {
      if (pop.hidden) return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        active = Math.min(active + 1, items.length - 1);
        markActive();
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        active = Math.max(active - 1, 0);
        markActive();
      } else if (e.key === "Enter" || e.key === "Tab") {
        if (active >= 0) {
          e.preventDefault();
          choose(active);
        }
      } else if (e.key === "Escape") {
        close();
      }
    });

    input.addEventListener("blur", () => setTimeout(close, 120));
  });
})();
