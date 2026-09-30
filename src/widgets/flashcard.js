(function () {
  'use strict';
  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    el.setAttribute('role', 'button');
    el.setAttribute('tabindex', '0');
    el.setAttribute('aria-pressed', 'false');
    function toggle() {
      el.classList.toggle('flipped');
      el.setAttribute('aria-pressed', el.classList.contains('flipped') ? 'true' : 'false');
    }
    el.addEventListener('click', toggle);
    el.addEventListener('keydown', function (e) {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        toggle();
      }
    });
  }
  function init() {
    document.querySelectorAll('[data-widget-type="flashcard"]').forEach(mount);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
