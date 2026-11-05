(function () {
  'use strict';
  var KATEX_CSS = 'https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css';
  var KATEX_JS = 'https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js';
  var MATH_EL = '.math-display[data-tex], .math-inline[data-tex], .step-tex[data-tex]';
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

  // Copying rendered KaTeX yields glyph soup from .katex-html, so rewrite the
  // clipboard text with the TeX source (kept on data-tex through rendering).
  function texOf(el) {
    return (el && el.dataset && el.dataset.tex) || '';
  }

  function delimited(el) {
    var display = el.classList.contains('math-display') || el.classList.contains('step-tex');
    return (display ? '$$' : '$') + texOf(el) + (display ? '$$' : '$');
  }

  function selectedMath(range) {
    var out = [];
    var els = document.querySelectorAll(MATH_EL);
    for (var i = 0; i < els.length; i++) {
      try {
        if (range.intersectsNode(els[i])) out.push(els[i]);
      } catch (err) { /* older engines */ }
    }
    return out;
  }

  function contains(container, el) {
    return container === el || (container && el.contains(container));
  }

  function hostFor(range, mathEls) {
    for (var i = 0; i < mathEls.length; i++) {
      if (contains(range.startContainer, mathEls[i]) || contains(range.endContainer, mathEls[i])) {
        return mathEls[i];
      }
    }
    return null;
  }

  function onCopy(e) {
    var sel = window.getSelection && window.getSelection();
    if (!e.clipboardData || !sel || sel.rangeCount === 0 || sel.isCollapsed) return;
    var range = sel.getRangeAt(0);
    var mathEls = selectedMath(range);
    if (!mathEls.length) return;

    // Whole selection inside one formula: the raw source is what the user wants.
    var startHost = hostFor({ startContainer: range.startContainer, endContainer: range.startContainer }, mathEls);
    var endHost = hostFor({ startContainer: range.endContainer, endContainer: range.endContainer }, mathEls);
    if (startHost && startHost === endHost && mathEls.length === 1) {
      e.clipboardData.setData('text/plain', texOf(startHost));
      e.preventDefault();
      return;
    }

    // Mixed selection: rebuild the text, substituting TeX for each math piece.
    // Cloned partial .katex pieces carry no data-tex; map them back to the
    // boundary host, falling back to KaTeX's own TeX annotation.
    var frag = range.cloneContents();
    var out = [];
    (function walk(node) {
      var children = Array.prototype.slice.call(node.childNodes);
      children.forEach(function (child) {
        if (child.nodeType === 3) { out.push(child.nodeValue); return; }
        if (child.nodeType !== 1) return;
        if (child.dataset && child.dataset.tex !== undefined) {
          out.push(delimited(child));
          return;
        }
        var isKatex = child.classList && child.classList.contains('katex');
        var katexIn = !isKatex && child.querySelector && child.querySelector('.katex');
        if (isKatex || katexIn) {
          var host = hostFor(range, mathEls);
          var annotation = child.querySelector && child.querySelector('annotation');
          if (host) out.push(delimited(host));
          else if (annotation) out.push(annotation.textContent);
          if (host || annotation) return;
        }
        walk(child);
      });
    })(frag);
    e.clipboardData.setData('text/plain', out.join(''));
    e.preventDefault();
  }

  function init() {
    mountIn(document);
    if (!window.__brillanteMathCopy) {
      window.__brillanteMathCopy = true;
      document.addEventListener('copy', onCopy);
    }
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
