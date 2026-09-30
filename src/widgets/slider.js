(function () {
  'use strict';
  function decimals(step) {
    var s = String(step);
    var i = s.indexOf('.');
    return i === -1 ? 0 : s.length - i - 1;
  }
  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    var config;
    try { config = JSON.parse(el.dataset.config || '{}'); } catch (e) { return; }
    var input = el.querySelector('input[type="range"]');
    var out = el.querySelector('output');
    if (!input || !out) return;
    var d = decimals(config.step === undefined ? 1 : config.step);
    function render(value) {
      var v = Number(value).toFixed(d);
      var text;
      if (config.display) {
        text = String(config.display).replace(/\{value\}/g, v);
      } else if (config.unit) {
        text = v + ' ' + config.unit;
      } else {
        text = v;
      }
      out.textContent = text;
    }
    input.addEventListener('input', function () { render(input.value); });
    render(input.value);
  }
  function init() {
    document.querySelectorAll('[data-widget-type="slider"]').forEach(mount);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
