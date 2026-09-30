(function () {
  'use strict';
  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
  }
  function init() {
    document.querySelectorAll('[data-widget-type="resource-link"]').forEach(mount);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();