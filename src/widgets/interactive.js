(function () {
  'use strict';
  var MARGIN = { top: 18, right: 16, bottom: 34, left: 44 };

  function evalAst(node, env) {
    if (!Array.isArray(node)) return NaN;
    var tag = node[0];
    if (tag === 'n') return node[1];
    if (tag === 'v') {
      var name = node[1];
      return name === 'x' ? env.x : (env[name] !== undefined ? env[name] : NaN);
    }
    if (tag === 'neg') return -evalAst(node[1], env);
    if (tag === 'f') {
      var fn = node[1];
      var a = node[2].map(function (arg) { return evalAst(arg, env); });
      switch (fn) {
        case 'exp': return Math.exp(a[0]);
        case 'log': return Math.log(a[0]);
        case 'sqrt': return Math.sqrt(a[0]);
        case 'abs': return Math.abs(a[0]);
        case 'min': return Math.min(a[0], a[1]);
        case 'max': return Math.max(a[0], a[1]);
        case 'pow': return Math.pow(a[0], a[1]);
        case 'sin': return Math.sin(a[0]);
        case 'cos': return Math.cos(a[0]);
        default: return NaN;
      }
    }
    var l = evalAst(node[1], env), r = evalAst(node[2], env);
    switch (tag) {
      case '+': return l + r;
      case '-': return l - r;
      case '*': return l * r;
      case '/': return l / r;
      case '^': return Math.pow(l, r);
      default: return NaN;
    }
  }

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

  function computeSeries(config, env) {
    var points = [];
    var xmin = config.domain[0], xmax = config.domain[1];
    var n = config.samples || 240;
    for (var i = 0; i <= n; i++) {
      var x = xmin + (xmax - xmin) * i / n;
      env.x = x;
      var y = evalAst(config.ast, env);
      points.push([x, isFinite(y) ? y : null]);
    }
    return points.filter(function (p) { return p[1] !== null; });
  }

  function draw(canvas, config, env, markerX) {
    var rect = canvas.getBoundingClientRect();
    var dpr = window.devicePixelRatio || 1;
    var w = Math.max(120, rect.width);
    var h = Math.max(80, rect.height);
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    var ctx = canvas.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    var points = computeSeries(config, env);
    if (!points.length) return;
    var minX = config.domain[0], maxX = config.domain[1];
    var ys = points.map(function (p) { return p[1]; });
    var minY, maxY;
    if (config.ydomain) {
      minY = config.ydomain[0]; maxY = config.ydomain[1];
      points = points.filter(function (p) { return p[1] >= minY && p[1] <= maxY; });
      if (!points.length) return;
    } else {
      minY = Math.min.apply(null, ys);
      maxY = Math.max.apply(null, ys);
    }
    if (minY === maxY) { minY -= 1; maxY += 1; }
    var padY = (maxY - minY) * 0.08;
    if (!config.ydomain) { minY -= padY; maxY += padY; }
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
    ctx.strokeStyle = config.color || '#6366f1';
    ctx.lineWidth = 2;
    ctx.lineJoin = 'round';
    ctx.beginPath();
    points.forEach(function (p, i) {
      if (i === 0) ctx.moveTo(px(p[0]), py(p[1]));
      else ctx.lineTo(px(p[0]), py(p[1]));
    });
    ctx.stroke();
    if (markerX !== undefined && isFinite(markerX)) {
      var yAt = evalAst(config.ast, Object.assign({}, env, { x: markerX }));
      if (isFinite(yAt) && (!config.ydomain || (yAt >= config.ydomain[0] && yAt <= config.ydomain[1]))) {
        ctx.fillStyle = '#ef4444';
        ctx.beginPath();
        ctx.arc(px(markerX), py(yAt), 5, 0, Math.PI * 2);
        ctx.fill();
        ctx.setLineDash([4, 4]);
        ctx.strokeStyle = '#ef4444';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(px(markerX), MARGIN.top);
        ctx.lineTo(px(markerX), h - MARGIN.bottom);
        ctx.stroke();
        ctx.setLineDash([]);
      }
    }
  }

  function mount(el) {
    if (el.dataset.mounted) return;
    el.dataset.mounted = 'true';
    var config;
    try { config = JSON.parse(el.dataset.config || '{}'); } catch (e) { return; }
    if (!config.ast || !config.domain) return;
    var canvas = el.querySelector('canvas');
    if (!canvas) return;
    var env = {};
    var inputs = Array.prototype.slice.call(el.querySelectorAll('input[type="range"]'));
    inputs.forEach(function (input) {
      var name = input.dataset.param;
      env[name] = Number(input.value);
      var out = el.querySelector('output[data-param="' + name + '"]');
      var decimals = 0, s = String(input.step || '1');
      var di = s.indexOf('.');
      if (di !== -1) decimals = s.length - di - 1;
      function onInput() {
        env[name] = Number(input.value);
        if (out) out.textContent = Number(input.value).toFixed(decimals);
        draw(canvas, config, env);
        if (config.readout) {
          var ro = el.querySelector('.interactive-readout');
          if (ro) {
            var text = config.readout;
            Object.keys(env).forEach(function (k) {
              text = text.split('{' + k + '}').join(Number(env[k]).toFixed(3));
            });
            ro.textContent = text;
          }
        }
      }
      input.addEventListener('input', onInput);
    });
    var schedule = function () { draw(canvas, config, env); };
    schedule();
    var raf = null;
    if (config.animate) {
      var t0 = null;
      var period = (config.animatePeriod || 6) * 1000;
      var xmin = config.domain[0], xmax = config.domain[1];
      var frame = function (ts) {
        if (t0 === null) t0 = ts;
        var phase = ((ts - t0) % period) / period;
        draw(canvas, config, env, xmin + (xmax - xmin) * phase);
        raf = window.requestAnimationFrame(frame);
      };
      raf = window.requestAnimationFrame(frame);
    }
    if (config.readout) {
      var ro = el.querySelector('.interactive-readout');
      if (ro) {
        var text = config.readout;
        Object.keys(env).forEach(function (k) {
          text = text.split('{' + k + '}').join(Number(env[k]).toFixed(3));
        });
        ro.textContent = text;
      }
    }
    if (window.ResizeObserver) {
      var ro = new ResizeObserver(function () { schedule(); });
      ro.observe(canvas);
    }
    window.addEventListener('pagehide', function () {
      if (raf) window.cancelAnimationFrame(raf);
    });
  }
  function init() {
    document.querySelectorAll('[data-widget-type="interactive"]').forEach(mount);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();