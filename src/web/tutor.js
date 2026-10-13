(() => {
  "use strict";

  document.addEventListener("DOMContentLoaded", () => {
    const panel = document.getElementById("tutor");
    const toggle = document.getElementById("tutor-toggle");
    if (!panel || !toggle || panel.dataset.mounted) return;
    panel.dataset.mounted = "1";

    const log = panel.querySelector("[data-tutor-log]");
    const input = panel.querySelector("[data-tutor-input]");
    const sendBtn = panel.querySelector("[data-tutor-send]");
    const closeBtn = panel.querySelector("[data-tutor-close]");
    const history = [];

    document.body.classList.add("tutor-drawer");
    toggle.hidden = false;

    function setOpen(open) {
      panel.classList.toggle("open", open);
      toggle.setAttribute("aria-expanded", String(open));
      if (open) input.focus();
    }

    toggle.addEventListener("click", () => setOpen(!panel.classList.contains("open")));
    closeBtn.addEventListener("click", () => setOpen(false));

    function bubble(role, text, html) {
      const p = document.createElement("p");
      p.className = "tutor-msg tutor-msg-" + role;
      if (role === "assistant" && html) {
        p.innerHTML = html;
        if (window.brillanteMath) window.brillanteMath(p);
      } else {
        p.textContent = text;
      }
      log.append(p);
      log.scrollTop = log.scrollHeight;
      return p;
    }

    async function send() {
      const question = input.value.trim();
      if (!question || sendBtn.disabled) return;
      input.value = "";
      history.push({ role: "user", content: question });
      bubble("user", question);
      const pending = bubble("assistant", "Thinking…");
      pending.classList.add("tutor-pending");
      sendBtn.disabled = true;
      try {
        const response = await fetch("/api/tutor/chat", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            course: document.body.dataset.course,
            lesson: document.body.dataset.lesson,
            messages: history,
          }),
        });
        const data = await response.json().catch(() => ({}));
        pending.remove();
        if (!response.ok || !data.reply) {
          const note = bubble("assistant", data.error || "The tutor is unavailable right now.");
          note.classList.add("tutor-error");
          history.pop();
        } else {
          history.push({ role: "assistant", content: data.reply });
          bubble("assistant", data.reply, data.html);
        }
      } catch {
        pending.remove();
        const note = bubble("assistant", "Could not reach the tutor.");
        note.classList.add("tutor-error");
        history.pop();
      } finally {
        sendBtn.disabled = false;
        input.focus();
      }
    }

    sendBtn.addEventListener("click", send);
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        send();
      }
    });
  });
})();
