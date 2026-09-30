(function () {
  'use strict';
  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    var config;
    try { config = JSON.parse(el.dataset.config || '{}'); } catch (e) { return; }
    var options = Array.prototype.slice.call(el.querySelectorAll('.quiz-option'));
    var explanation = el.querySelector('.quiz-explanation');
    var attempts = 0;
    var solved = false;
    var hasExplanation = explanation && explanation.textContent.trim().length > 0;
    options.forEach(function (btn) {
      btn.addEventListener('click', function () {
        if (solved) return;
        var index = parseInt(btn.dataset.index, 10);
        if (index === config.answer) {
          solved = true;
          btn.classList.add('correct');
          options.forEach(function (b) { b.disabled = true; });
          el.classList.add('quiz-solved');
          if (hasExplanation) explanation.hidden = false;
        } else {
          attempts += 1;
          btn.classList.add('incorrect');
          btn.disabled = true;
          if (attempts >= options.length - 1) {
            var correct = el.querySelector('.quiz-option[data-index="' + config.answer + '"]');
            if (correct) correct.classList.add('reveal');
          }
        }
      });
    });
  }
  function init() {
    document.querySelectorAll('[data-widget-type="quiz"]').forEach(mount);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
