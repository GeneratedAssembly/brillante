(function () {
  'use strict';
  var KATEX_CSS = 'https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css';
  var KATEX_JS = 'https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js';
  var pending = [];

  function loadKatex(cb) {
    if (window.katex) { cb(); return; }
    pending.push(cb);
    if (pending.length > 1) return;
    var link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = KATEX_CSS;
    document.head.appendChild(link);
    var script = document.createElement('script');
    script.src = KATEX_JS;
    script.onload = function () { pending.splice(0).forEach(function (f) { f(); }); };
    script.onerror = function () { pending.splice(0).forEach(function (f) { f(); }); };
    document.head.appendChild(script);
  }

  function render(el) {
    var tex = el.dataset.tex || '';
    if (window.katex) {
      try {
        window.katex.render(tex, el, { displayMode: true, throwOnError: false, trust: false, strict: false });
        return;
      } catch (e) { /* fall through */ }
    }
    el.textContent = tex;
    el.classList.add('math-fallback');
  }

  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    loadKatex(function () { render(el); });
  }

  function mountIn(root) {
    (root || document).querySelectorAll('.math-display[data-tex], .step-tex[data-tex]').forEach(mount);
    (root || document).querySelectorAll('.math-inline[data-tex]').forEach(function (el) {
      if (el.dataset.mounted) return;
      el.dataset.mounted = 'true';
      loadKatex(function () {
        var tex = el.dataset.tex || '';
        if (window.katex) {
          try {
            window.katex.render(tex, el, { displayMode: false, throwOnError: false, trust: false, strict: false });
            return;
          } catch (e) { /* fall through */ }
        }
        el.textContent = tex;
        el.classList.add('math-fallback');
      });
    });
  }

  // Re-render math inside dynamically injected fragments (tutor chat, notes
  // preview). The initial pass below covers server-rendered pages.
  window.brillanteMath = mountIn;

  function init() {
    mountIn(document);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
