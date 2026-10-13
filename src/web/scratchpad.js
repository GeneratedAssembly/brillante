(function () {
  'use strict';

  var PYODIDE_VERSION = '0.26.4';
  var PYODIDE_BASE = 'https://cdn.jsdelivr.net/pyodide/v' + PYODIDE_VERSION + '/full/';
  var HISTORY_KEY = 'brillante.scratch.history.v1';

  var BOOTSTRAP = [
    'from pyodide.console import PyodideConsole, repr_shorten',
    'console = PyodideConsole()',
  ].join('\n');

  var box, toggle, log, input, promptLabel, statusEl, form;
  var resetBtn, closeBtn, runBtn;
  var pyodide = null;
  var pyconsole = null;
  var reprFn = null;
  var booting = false;
  var booted = false;
  var execCount = 0;
  var continuation = false;
  var busy = false;
  var history = [];
  var historyPos = -1;
  var draft = '';

  function tryDestroy(proxy) {
    try {
      if (proxy && typeof proxy.destroy === 'function') proxy.destroy();
    } catch (e) { /* already destroyed */ }
  }

  function setStatus(state, text) {
    statusEl.dataset.state = state;
    statusEl.textContent = text;
  }

  function setPrompt() {
    promptLabel.textContent = continuation ? '...:' : 'In [' + (execCount + 1) + ']:';
  }

  function addRow(kind, text) {
    var row = document.createElement('div');
    row.className = 'scratchpad-row scratchpad-' + kind;
    if (text) row.textContent = text;
    log.appendChild(row);
    log.scrollTop = log.scrollHeight;
    return row;
  }

  function echo(source) {
    var promptText = continuation ? '...:' : 'In [' + (execCount + 1) + ']:';
    var row = document.createElement('div');
    row.className = 'scratchpad-row scratchpad-in';
    var mark = document.createElement('span');
    mark.className = 'scratchpad-prompt-part';
    mark.textContent = promptText + ' ';
    var body = document.createElement('span');
    var lines = source.split('\n');
    var rest = [];
    for (var i = 1; i < lines.length; i++) rest.push('...: ' + lines[i]);
    body.textContent = lines[0] + (lines.length > 1 ? '\n' : '') + rest.join('\n');
    row.appendChild(mark);
    row.appendChild(body);
    log.appendChild(row);
    log.scrollTop = log.scrollHeight;
  }

  function autosize() {
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 224) + 'px';
  }

  function loadHistory() {
    try {
      var raw = JSON.parse(localStorage.getItem(HISTORY_KEY) || '[]');
      if (Array.isArray(raw)) {
        history = raw.filter(function (s) { return typeof s === 'string'; }).slice(-400);
      }
    } catch (e) {
      history = [];
    }
  }

  function saveHistory() {
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(history.slice(-400)));
    } catch (e) { /* storage unavailable */ }
  }

  function rememberHistory(source) {
    if (history[history.length - 1] === source) return;
    history.push(source);
    saveHistory();
  }

  function historyPrev() {
    if (!history.length || busy) return;
    if (historyPos === -1) {
      draft = input.value;
      historyPos = history.length - 1;
    } else if (historyPos > 0) {
      historyPos -= 1;
    } else {
      return;
    }
    input.value = history[historyPos];
    input.setSelectionRange(input.value.length, input.value.length);
    autosize();
  }

  function historyNext() {
    if (historyPos === -1) return;
    historyPos += 1;
    if (historyPos >= history.length) {
      historyPos = -1;
      input.value = draft;
    } else {
      input.value = history[historyPos];
    }
    var end = input.value.length;
    input.setSelectionRange(end, end);
    autosize();
  }

  function focusInput() {
    if (!input.disabled) input.focus();
  }

  function submit() {
    if (!booted || busy) return;
    var source = input.value.replace(/\n+$/, '');
    if (!continuation && !source.trim()) return;
    if (source) echo(source);
    if (source) rememberHistory(source);
    input.value = '';
    autosize();
    busy = true;
    setStatus('running', 'running…');
    exec(source);
  }

  async function exec(source) {
    var result = null;
    try {
      result = pyconsole.push(source);
      var status = String(result.syntax_check || result.status || '');
      if (status === 'incomplete') {
        continuation = true;
        setPrompt();
        tryDestroy(result);
        busy = false;
        setStatus('ready', 'ready');
        focusInput();
        return;
      }
      continuation = false;
      setPrompt();
      var formattedError = '';
      try { formattedError = String(result.formatted_error || ''); } catch (e) { }
      var value;
      try {
        value = await result;
      } catch (pyerr) {
        tryDestroy(result);
        addRow('err', tidyTraceback(formattedError || ((pyerr && pyerr.message) || String(pyerr))));
        execCount += 1;
        setPrompt();
        busy = false;
        setStatus('ready', 'ready');
        focusInput();
        return;
      }
      tryDestroy(result);
      if (status === 'syntax-error') {
        addRow('err', formattedError || 'SyntaxError');
      } else if (value !== undefined && value !== null) {
        var repr = '';
        try { repr = String(reprFn(value)); } catch (e) { }
        if (repr) addRow('out', 'Out [' + (execCount + 1) + ']: ' + repr);
      }
      execCount += 1;
      setPrompt();
      busy = false;
      setStatus('ready', 'ready');
      focusInput();
    } catch (e) {
      busy = false;
      setStatus('error', 'error');
      tryDestroy(result);
      addRow('err', 'scratch pad error: ' + ((e && e.message) || e));
    }
  }

  function cancelInput() {
    if (busy) {
      addRow('info', 'a running statement cannot be interrupted — refresh the page to reset');
      return;
    }
    if (continuation && pyconsole && pyconsole.resetbuffer) {
      try { pyconsole.resetbuffer(); } catch (e) { }
    }
    continuation = false;
    input.value = '';
    autosize();
    setPrompt();
    addRow('info', 'KeyboardInterrupt');
    focusInput();
  }

  function completeAtCursor() {
    if (!booted || busy || !pyconsole) return;
    var pos = input.selectionStart;
    if (pos !== input.selectionEnd) return;
    var before = input.value.slice(0, pos);
    if (!before.trim()) return;
    var pair = pyconsole.complete(before);
    var completions = [];
    var start = pos;
    try {
      var arr = pair.toJs();
      var list = arr[0];
      start = Number(arr[1]);
      for (var i = 0; i < list.length; i++) completions.push(String(list[i]));
    } catch (e) {
      completions = [];
    } finally {
      tryDestroy(pair);
    }
    if (!completions.length) return;
    var word = before.slice(start);
    var prefix = completions[0];
    for (var j = 1; j < completions.length; j++) {
      while (prefix && completions[j].indexOf(prefix) !== 0) prefix = prefix.slice(0, -1);
    }
    var insertion = prefix.slice(word.length);
    if (insertion) {
      input.value = input.value.slice(0, pos) + insertion + input.value.slice(pos);
      input.setSelectionRange(pos + insertion.length, pos + insertion.length);
      autosize();
    }
    if (completions.length > 1) addRow('out', completions.join('   '));
  }

  function tidyTraceback(text) {
    if (text.indexOf('Traceback (most recent call last)') === -1) return text;
    var idx = text.indexOf('File "<console>"');
    if (idx === -1) return text;
    return 'Traceback (most recent call last):\n' + text.slice(idx);
  }

  function banner() {
    var ver = '';
    try { ver = String(pyodide.runPython('import sys; sys.version.split()[0]')); } catch (e) { }
    addRow('info', 'Python ' + (ver || '3.x') + ' (Pyodide ' + PYODIDE_VERSION + ', WebAssembly) — sandboxed in your browser, nothing is sent to any server.');
    addRow('info', 'Bundled packages (numpy, pandas, matplotlib, …) import automatically. For others: import micropip; await micropip.install("pkg")');
    addRow('info', 'Enter runs (a blank line ends a block) · Shift+Enter newline · Tab completes · ↑/↓ history · Ctrl+C cancels input. State resets when you leave the page.');
  }

  function markReady() {
    booted = true;
    booting = false;
    input.disabled = false;
    if (runBtn) runBtn.disabled = false;
    if (resetBtn) resetBtn.disabled = false;
    setPrompt();
    addRow('info', 'ready.');
    banner();
    setStatus('ready', 'ready');
    focusInput();
  }

  function ensureKernel() {
    if (booted || booting) return;
    booting = true;
    setStatus('loading', 'loading…');
    addRow('info', 'loading Python (~10 MB on first use, cached afterwards)…');
    var script = document.createElement('script');
    script.src = PYODIDE_BASE + 'pyodide.js';
    script.onload = function () {
      window.loadPyodide({ indexURL: PYODIDE_BASE })
        .then(function (py) {
          pyodide = py;
          pyodide.setStdout({ batched: function (s) { addRow('out', s); } });
          pyodide.setStderr({ batched: function (s) { addRow('err', s); } });
          return py.runPythonAsync(BOOTSTRAP);
        })
        .then(function () {
          pyconsole = pyodide.globals.get('console');
          reprFn = pyodide.globals.get('repr_shorten');
          markReady();
        })
        .catch(function (err) {
          booting = false;
          setStatus('error', 'error');
          addRow('err', 'could not start Python: ' + ((err && err.message) || err));
        });
    };
    script.onerror = function () {
      booting = false;
      setStatus('error', 'offline');
      addRow('err', 'the Python runtime could not be downloaded (offline?). The scratch pad needs network access on first use.');
    };
    document.head.appendChild(script);
  }

  function resetKernel() {
    if (!booted) return;
    tryDestroy(pyconsole);
    try {
      pyodide.runPython('console = PyodideConsole()');
      pyconsole = pyodide.globals.get('console');
      execCount = 0;
      continuation = false;
      log.textContent = '';
      banner();
      setPrompt();
      setStatus('ready', 'ready');
      focusInput();
    } catch (e) {
      setStatus('error', 'error');
      addRow('err', 'reset failed: ' + ((e && e.message) || e));
    }
  }

  function setOpen(open) {
    var isOpen = box.classList.contains('open');
    if (open === isOpen) return;
    box.classList.toggle('open', open);
    toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
    if (open) {
      ensureKernel();
      focusInput();
    }
  }

  function init() {
    box = document.getElementById('scratchpad');
    toggle = document.getElementById('scratchpad-toggle');
    if (!box || !toggle || box.dataset.mounted) return;
    box.dataset.mounted = '1';
    log = box.querySelector('[data-scratch-log]');
    input = box.querySelector('[data-scratch-input]');
    promptLabel = box.querySelector('[data-scratch-prompt]');
    statusEl = box.querySelector('[data-scratch-status]');
    form = box.querySelector('[data-scratch-form]');
    resetBtn = box.querySelector('[data-scratch-reset]');
    closeBtn = box.querySelector('[data-scratch-close]');
    runBtn = box.querySelector('[data-scratch-run]');
    if (!log || !input || !promptLabel || !statusEl || !form) return;

    loadHistory();
    setPrompt();
    input.setAttribute('spellcheck', 'false');
    input.setAttribute('autocapitalize', 'off');
    input.setAttribute('autocomplete', 'off');
    toggle.hidden = false;
    toggle.addEventListener('click', function () {
      setOpen(!box.classList.contains('open'));
    });
    if (closeBtn) closeBtn.addEventListener('click', function () { setOpen(false); });
    if (resetBtn) resetBtn.addEventListener('click', resetKernel);
    if (runBtn) runBtn.disabled = true;
    form.addEventListener('submit', function (e) {
      e.preventDefault();
      submit();
    });
    input.addEventListener('input', autosize);
    input.addEventListener('keydown', function (e) {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        submit();
        return;
      }
      if (e.key === 'Tab') {
        e.preventDefault();
        completeAtCursor();
        return;
      }
      if (e.key === 'ArrowUp' && !e.shiftKey && !continuation &&
          input.selectionStart === 0 && input.selectionEnd === 0) {
        e.preventDefault();
        historyPrev();
        return;
      }
      if (e.key === 'ArrowDown' && !e.shiftKey && !continuation &&
          input.selectionEnd === input.value.length) {
        e.preventDefault();
        historyNext();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === 'c' || e.key === 'C') &&
          input.selectionStart === input.selectionEnd) {
        e.preventDefault();
        cancelInput();
      }
    });
    box.addEventListener('keydown', function (e) {
      if (e.key === 'Escape') setOpen(false);
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
