(function () {
  'use strict';
  var DRAFT_PREFIX = 'brillante.teach.v1.';
  var MAX = 8000;

  function draftKey(id) { return DRAFT_PREFIX + id; }

  function readDraft(id) {
    try { return window.localStorage.getItem(draftKey(id)) || ''; } catch (e) { return ''; }
  }

  function writeDraft(id, value) {
    try {
      if (value) window.localStorage.setItem(draftKey(id), value);
      else window.localStorage.removeItem(draftKey(id));
    } catch (e) { /* storage unavailable: drafts are best-effort */ }
  }

  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    var id = el.dataset.widgetId || '';
    var input = el.querySelector('[data-teach-input]');
    var counter = el.querySelector('[data-teach-count]');
    var previewBtn = el.querySelector('[data-teach-preview]');
    var previewOut = el.querySelector('[data-teach-preview-out]');
    var submitBtn = el.querySelector('[data-teach-submit]');
    var result = el.querySelector('[data-teach-result]');
    var verdictEl = el.querySelector('[data-teach-verdict]');
    var feedbackEl = el.querySelector('[data-teach-feedback]');
    var kpToggle = el.querySelector('[data-teach-key-points-toggle]');
    var kpBody = el.querySelector('[data-teach-key-points-body]');
    var errorEl = el.querySelector('[data-teach-error]');
    var attemptsEl = el.querySelector('[data-teach-attempts]');
    if (!id || !input || !counter || !previewBtn || !previewOut || !submitBtn || !result ||
        !verdictEl || !feedbackEl || !kpToggle || !kpBody || !errorEl) return;

    var course = document.body.dataset.course || '';
    var lesson = document.body.dataset.lesson || '';
    var signedIn = document.body.dataset.auth === 'server';
    var kpLoaded = false;

    if (!signedIn) {
      submitBtn.disabled = true;
      submitBtn.textContent = 'Sign in to grade';
    }

    function showError(msg) {
      errorEl.textContent = msg;
      errorEl.hidden = !msg;
    }

    function updateCount() {
      counter.textContent = input.value.length + ' / ' + MAX;
    }

    var saveTimer = null;
    input.addEventListener('input', function () {
      updateCount();
      if (saveTimer) window.clearTimeout(saveTimer);
      saveTimer = window.setTimeout(function () { writeDraft(id, input.value); }, 400);
    });

    function renderMath(root) {
      if (window.brillanteMath) window.brillanteMath(root);
    }

    function showResult(verdict, score, feedbackHtml, attempt) {
      var label = 'Needs work';
      if (verdict === 'correct') label = 'Clear and correct';
      else if (verdict === 'partially_correct') label = 'Partially there';
      verdictEl.textContent = label + ' · ' + Math.round(score * 100) + '%';
      verdictEl.className = 'teach-verdict teach-verdict-' + verdict;
      feedbackEl.innerHTML = feedbackHtml;
      renderMath(feedbackEl);
      result.hidden = false;
      if (attemptsEl && attempt) {
        attemptsEl.textContent = 'Attempt ' + attempt;
        attemptsEl.hidden = false;
      }
    }

    function unlockKeyPoints(html) {
      if (!html) return;
      kpBody.innerHTML = html;
      renderMath(kpBody);
      kpLoaded = true;
      kpToggle.disabled = false;
    }

    kpToggle.addEventListener('click', function () {
      if (!kpLoaded) return;
      var show = kpBody.hidden;
      kpBody.hidden = !show;
      kpToggle.textContent = show ? 'Hide key points' : 'Show key points';
    });

    previewBtn.addEventListener('click', function () {
      if (!previewOut.hidden) {
        previewOut.hidden = true;
        previewBtn.textContent = 'Preview';
        return;
      }
      showError('');
      previewBtn.disabled = true;
      fetch('/api/teach/preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ course: course, lesson: lesson, body: input.value }),
      })
        .then(function (r) { return r.ok ? r.json() : Promise.reject(new Error(String(r.status))); })
        .then(function (data) {
          previewOut.innerHTML = data.html || '';
          renderMath(previewOut);
          previewOut.hidden = false;
          previewBtn.textContent = 'Hide preview';
        })
        .catch(function () { showError('Preview is unavailable right now.'); })
        .finally(function () { previewBtn.disabled = false; });
    });

    submitBtn.addEventListener('click', function () {
      if (!signedIn) return;
      var explanation = input.value;
      if (!explanation.trim()) {
        showError('Write an explanation before submitting.');
        return;
      }
      showError('');
      submitBtn.disabled = true;
      var submitLabel = submitBtn.textContent;
      submitBtn.textContent = 'Grading…';
      fetch('/api/teach/grade', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ course: course, lesson: lesson, widget: id, explanation: explanation }),
      })
        .then(function (r) {
          return r.json().then(function (data) {
            if (!r.ok) throw new Error(data.error || String(r.status));
            return data;
          });
        })
        .then(function (data) {
          showResult(data.verdict, data.score, data.feedback_html || '', data.attempt);
          unlockKeyPoints(data.key_points_html || '');
          writeDraft(id, '');
        })
        .catch(function (e) {
          showError(e.message || 'Grading is unavailable right now. Please try again.');
        })
        .finally(function () {
          submitBtn.disabled = false;
          submitBtn.textContent = submitLabel;
        });
    });

    // Restore: signed-in visitors get their persisted submission (which also
    // re-unlocks feedback and the key points); everyone gets the local draft.
    if (signedIn) {
      var params = '?course=' + encodeURIComponent(course) +
        '&lesson=' + encodeURIComponent(lesson) +
        '&widget=' + encodeURIComponent(id);
      fetch('/api/teach/submission' + params)
        .then(function (r) { return r.ok ? r.json() : null; })
        .then(function (data) {
          if (!data) {
            var draft = readDraft(id);
            if (draft) input.value = draft;
            updateCount();
            return;
          }
          input.value = data.answer || '';
          updateCount();
          showResult(data.verdict, data.score || 0, data.feedback_html || '', data.attempt);
          unlockKeyPoints(data.key_points_html || '');
          writeDraft(id, '');
        })
        .catch(function () {
          var draft = readDraft(id);
          if (draft) input.value = draft;
          updateCount();
        });
    } else {
      var draft = readDraft(id);
      if (draft) input.value = draft;
      updateCount();
    }
  }

  function init() {
    document.querySelectorAll('[data-widget-type="teach"]').forEach(mount);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
