(function () {
  'use strict';
  var KEY = 'brillante.progress.v1';
  var ACK_KEY = 'brillante.ack.v1';

  function loadKey(key) {
    try {
      var raw = JSON.parse(localStorage.getItem(key) || '{}');
      return raw && typeof raw === 'object' ? raw : {};
    } catch (e) {
      return {};
    }
  }

  function load() {
    return loadKey(KEY);
  }

  function loadAcks() {
    return loadKey(ACK_KEY);
  }

  function saveKey(key, data) {
    try {
      localStorage.setItem(key, JSON.stringify(data));
    } catch (e) { /* storage unavailable */ }
  }

  function save(data) {
    saveKey(KEY, data);
  }

  function isDone(data, course, lesson) {
    return !!(data[course] && data[course].indexOf(lesson) !== -1);
  }

  function setDone(data, course, lesson, done) {
    var list = data[course] || [];
    var i = list.indexOf(lesson);
    if (done && i === -1) {
      list.push(lesson);
    } else if (!done && i !== -1) {
      list.splice(i, 1);
    }
    if (list.length) {
      data[course] = list;
    } else {
      delete data[course];
    }
    save(data);
  }

  function serverAuth() {
    return document.body.dataset.auth === 'server';
  }

  function paintButton(btn, done) {
    btn.classList.toggle('done', done);
    btn.textContent = done ? 'Completed' : 'Mark complete';
  }

  function postProgress(course, lesson, done, cb) {
    var xhr = new XMLHttpRequest();
    xhr.open('POST', '/api/progress', true);
    xhr.setRequestHeader('Content-Type', 'application/json');
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== 4) return;
      cb(xhr.status >= 200 && xhr.status < 300, xhr.status);
    };
    xhr.send(JSON.stringify({ course: course, lesson: lesson, done: done }));
  }

  function initLesson() {
    var btn = document.getElementById('mark-complete');
    if (!btn) return;
    var course = btn.dataset.course;
    var lesson = btn.dataset.lesson;
    var server = serverAuth();
    var pending = false;
    function currentDone() {
      return server ? btn.classList.contains('done') : isDone(load(), course, lesson);
    }
    paintButton(btn, currentDone());
    btn.addEventListener('click', function () {
      if (pending) return;
      var done = !currentDone();
      if (server) {
        pending = true;
        btn.disabled = true;
        postProgress(course, lesson, done, function (ok, status) {
          pending = false;
          btn.disabled = false;
          if (ok) {
            paintButton(btn, done);
          } else if (status === 401) {
            window.location.href = '/account?next=' + encodeURIComponent(window.location.pathname);
          }
        });
      } else {
        var data = load();
        setDone(data, course, lesson, done);
        paintButton(btn, done);
      }
    });
  }

  function initCourse() {
    var items = document.querySelectorAll('.lesson-item[data-lesson]');
    if (!items.length) return;
    var course = document.body.dataset.course;
    if (!course) return;
    if (serverAuth()) return; // server rendered the completed state
    var data = load();
    var done = 0;
    items.forEach(function (li) {
      var finished = isDone(data, course, li.dataset.lesson);
      li.classList.toggle('done', finished);
      if (finished) done += 1;
    });
    var counter = document.querySelector('[data-progress-count]');
    if (counter) counter.textContent = String(done);
  }

  function initAckBanner() {
    var banner = document.querySelector('.ack-banner');
    if (!banner) return;
    var ids = [];
    try {
      var parsed = JSON.parse(banner.dataset.ackIds || '[]');
      if (Array.isArray(parsed)) ids = parsed;
    } catch (e) { ids = []; }
    if (!ids.length) return;
    function acknowledged() {
      var data = loadAcks();
      for (var i = 0; i < ids.length; i++) {
        if (data[ids[i]]) return true;
      }
      return false;
    }
    function paint() {
      banner.hidden = acknowledged();
    }
    paint();
    document.addEventListener('brillante:ack', paint);
    window.addEventListener('storage', function (e) {
      if (!e.key || e.key === ACK_KEY) paint();
    });
  }

  function initAccount() {
    var link = document.getElementById('account-link');
    if (!link || !serverAuth()) return;
    var xhr = new XMLHttpRequest();
    xhr.open('GET', '/auth/me', true);
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== 4 || xhr.status !== 200) return;
      var me;
      try { me = JSON.parse(xhr.responseText); } catch (e) { return; }
      if (me && me.authenticated && me.name) {
        link.textContent = me.name;
        link.title = 'Signed in as ' + me.name;
      }
    };
    xhr.send();
  }

  function init() {
    initLesson();
    initCourse();
    initAckBanner();
    initAccount();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
