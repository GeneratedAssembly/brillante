(function () {
  'use strict';
  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    var steps = Array.prototype.slice.call(el.querySelectorAll('.stepper-step'));
    var prev = el.querySelector('.stepper-prev');
    var next = el.querySelector('.stepper-next');
    var counter = el.querySelector('.stepper-counter');
    if (steps.length < 2 || !prev || !next || !counter) return;
    var index = 0;
    function show(i) {
      index = Math.max(0, Math.min(steps.length - 1, i));
      steps.forEach(function (s, k) {
        s.hidden = k !== index;
        s.classList.toggle('stepper-active', k === index);
      });
      counter.textContent = 'Step ' + (index + 1) + ' of ' + steps.length;
      prev.disabled = index === 0;
      next.disabled = index === steps.length - 1;
    }
    prev.addEventListener('click', function () { show(index - 1); });
    next.addEventListener('click', function () { show(index + 1); });
    el.setAttribute('tabindex', '0');
    el.addEventListener('keydown', function (e) {
      if (e.key === 'ArrowRight') { show(index + 1); e.preventDefault(); }
      if (e.key === 'ArrowLeft') { show(index - 1); e.preventDefault(); }
    });
    show(0);
  }
  function init() {
    document.querySelectorAll('[data-widget-type="stepper"]').forEach(mount);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();