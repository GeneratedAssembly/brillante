(function () {
  'use strict';
  var MARGIN = { top: 18, right: 16, bottom: 34, left: 44 };
  function niceStep(range) {
    var raw = range / 5;
    var mag = Math.pow(10, Math.floor(Math.log10(raw || 1)));
    var norm = raw / mag;
    var step;
    if (norm >= 5) step = 10;
    else if (norm >= 2) step = 5;
    else if (norm >= 1) step = 2;
    else step = 1;
    return step * mag;
  }
  function tickFormat(v, step) {
    if (Math.abs(v) < 1e-12) return '0';
    var decimals = Math.max(0, Math.ceil(-Math.log10(step)) + 1);
    return v.toFixed(Math.min(decimals, 6));
  }
  function draw(canvas, config) {
    var rect = canvas.getBoundingClientRect();
    var dpr = window.devicePixelRatio || 1;
    var w = Math.max(120, rect.width);
    var h = Math.max(80, rect.height);
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    var ctx = canvas.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    var series = config.series || [];
    var xs = [], ys = [];
    series.forEach(function (s) {
      (s.points || []).forEach(function (p) { xs.push(p[0]); ys.push(p[1]); });
    });
    if (!xs.length) return;
    var minX = Math.min.apply(null, xs), maxX = Math.max.apply(null, xs);
    var minY = Math.min.apply(null, ys), maxY = Math.max.apply(null, ys);
    if (minX === maxX) { minX -= 1; maxX += 1; }
    if (minY === maxY) { minY -= 1; maxY += 1; }
    var padX = (maxX - minX) * 0.04, padY = (maxY - minY) * 0.08;
    minX -= padX; maxX += padX; minY -= padY; maxY += padY;
    var px = function (x) {
      return MARGIN.left + (x - minX) / (maxX - minX) * (w - MARGIN.left - MARGIN.right);
    };
    var py = function (y) {
      return h - MARGIN.bottom - (y - minY) / (maxY - minY) * (h - MARGIN.top - MARGIN.bottom);
    };
    var xStep = niceStep(maxX - minX), yStep = niceStep(maxY - minY);
    ctx.strokeStyle = '#e2e8f0';
    ctx.lineWidth = 1;
    ctx.font = '11px system-ui, sans-serif';
    ctx.fillStyle = '#64748b';
    for (var x = Math.ceil(minX / xStep) * xStep; x <= maxX; x += xStep) {
      var gx = px(x);
      ctx.beginPath(); ctx.moveTo(gx, MARGIN.top); ctx.lineTo(gx, h - MARGIN.bottom); ctx.stroke();
      ctx.textAlign = 'center';
      ctx.fillText(tickFormat(x, xStep), gx, h - MARGIN.bottom + 14);
    }
    for (var y = Math.ceil(minY / yStep) * yStep; y <= maxY; y += yStep) {
      var gy = py(y);
      ctx.beginPath(); ctx.moveTo(MARGIN.left, gy); ctx.lineTo(w - MARGIN.right, gy); ctx.stroke();
      ctx.textAlign = 'right';
      ctx.fillText(tickFormat(y, yStep), MARGIN.left - 6, gy + 4);
    }
    ctx.strokeStyle = '#94a3b8';
    ctx.beginPath();
    ctx.moveTo(MARGIN.left, MARGIN.top); ctx.lineTo(MARGIN.left, h - MARGIN.bottom); ctx.lineTo(w - MARGIN.right, h - MARGIN.bottom);
    ctx.stroke();
    series.forEach(function (s) {
      ctx.strokeStyle = s.color || '#6366f1';
      ctx.lineWidth = 2;
      ctx.lineJoin = 'round';
      ctx.beginPath();
      (s.points || []).forEach(function (p, i) {
        if (i === 0) ctx.moveTo(px(p[0]), py(p[1]));
        else ctx.lineTo(px(p[0]), py(p[1]));
      });
      ctx.stroke();
      ctx.fillStyle = s.color || '#6366f1';
      (s.points || []).forEach(function (p) {
        ctx.beginPath();
        ctx.arc(px(p[0]), py(p[1]), 3, 0, Math.PI * 2);
        ctx.fill();
      });
    });
  }
  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    var config;
    try { config = JSON.parse(el.dataset.config || '{}'); } catch (e) { return; }
    var canvas = el.querySelector('canvas');
    if (!canvas) return;
    var schedule;
    schedule = function () { draw(canvas, config); };
    schedule();
    if (window.ResizeObserver) {
      var ro = new ResizeObserver(function () { schedule(); });
      ro.observe(canvas);
    }
  }
  function init() {
    document.querySelectorAll('[data-widget-type="plot"]').forEach(mount);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
