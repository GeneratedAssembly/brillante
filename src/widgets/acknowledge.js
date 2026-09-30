(function () {
  'use strict';
  var KEY = 'brillante.ack.v1';

  function load() {
    try {
      var raw = JSON.parse(localStorage.getItem(KEY) || '{}');
      return raw && typeof raw === 'object' ? raw : {};
    } catch (e) {
      return {};
    }
  }

  function save(data) {
    try {
      localStorage.setItem(KEY, JSON.stringify(data));
    } catch (e) { /* storage unavailable */ }
  }

  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    var id = el.dataset.widgetId;
    if (!id) return;
    var check = el.querySelector('.ack-check');
    var button = el.querySelector('.ack-button');
    var done = el.querySelector('.ack-done');
    var cfg = {};
    try {
      cfg = JSON.parse(el.dataset.config || '{}');
    } catch (e) { cfg = {}; }
    var doneMessage =
      typeof cfg.done_message === 'string' && cfg.done_message
        ? cfg.done_message
        : 'Acknowledged';

    function paint() {
      var acked = !!load()[id];
      el.classList.toggle('acked', acked);
      if (acked) {
        done.textContent = doneMessage;
        done.hidden = false;
        if (check) { check.checked = true; check.disabled = true; }
        if (button) button.disabled = true;
      } else {
        done.hidden = true;
        if (check) check.disabled = false;
        if (button) button.disabled = !(check && check.checked);
      }
    }

    function acknowledge() {
      var data = load();
      data[id] = true;
      save(data);
      paint();
      try {
        document.dispatchEvent(new CustomEvent('brillante:ack', { detail: { id: id } }));
      } catch (e) { /* older browsers */ }
    }

    if (check) {
      check.addEventListener('change', function () {
        if (button) button.disabled = !check.checked;
      });
    }
    if (button) button.addEventListener('click', acknowledge);
    paint();
  }

  function init() {
    document.querySelectorAll('[data-widget-type="acknowledge"]').forEach(mount);
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();