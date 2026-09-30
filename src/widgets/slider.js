(function () {
  'use strict';
  function decimals(step) {
    var s = String(step);
    var i = s.indexOf('.');
    return i === -1 ? 0 : s.length - i - 1;
  }
  // Mirrors expr::eval_ast in expr.rs (the client receives the parsed AST,
  // never the source expression; no eval/new Function anywhere).
  function evalAst(node, env) {
    if (!Array.isArray(node)) return NaN;
    var tag = node[0];
    if (tag === 'n') return node[1];
    if (tag === 'v') return env[node[1]] !== undefined ? env[node[1]] : NaN;
    if (tag === 'neg') return -evalAst(node[1], env);
    if (tag === 'f') {
      var a = node[2].map(function (arg) { return evalAst(arg, env); });
      switch (node[1]) {
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
  function specDecimals(spec) {
    var s = String(spec).replace(/f\s*$/, '');
    var i = s.lastIndexOf('.');
    var d = parseInt(i === -1 ? s : s.slice(i + 1), 10);
    return isNaN(d) ? null : d;
  }
  function fmt(v, dec) {
    return dec === 0 ? String(Math.round(v)) : v.toFixed(dec);
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
      var v = Number(value);
      var text;
      if (config.display) {
        var result = config.readoutAst ? evalAst(config.readoutAst, { x: v }) : null;
        text = String(config.display).replace(
          /\{([A-Za-z_][A-Za-z0-9_]*)(?::([^{}]+))?\}/g,
          function (m, name, spec) {
            var dec = spec === undefined ? null : specDecimals(spec);
            if (name === 'value') return fmt(v, dec === null ? d : dec);
            if (name === 'result') {
              return result !== null && isFinite(result) ? fmt(result, dec === null ? 2 : dec) : '—';
            }
            return m;
          }
        );
      } else if (config.unit) {
        text = v.toFixed(d) + ' ' + config.unit;
      } else {
        text = v.toFixed(d);
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