// markdown-viewer: renders ```mermaid blocks with the bundled mermaid.js.
// Registers itself in window.mdvEnhancers, which live.js runs on updated
// content *before* swapping it in, so the page doesn't flicker.
(function () {
  var nonce = document.currentScript && document.currentScript.nonce;
  var src = document.currentScript && document.currentScript.dataset.mermaidSrc;
  // Against the page's own address, never a <base> in the document.
  if (src) src = new URL(src, location.href).href;
  var cache = new Map();
  var version = src ? (src.split("?v=")[1] || "") : "inline";
  // Start once the page has been drawn, so its text shows before a library
  // loads and runs (at most 500 ms later, e.g. in a background tab).
  function afterPaint() {
    return new Promise(function (resolve) {
      // A frame's callback runs before it's drawn; the next one, after.
      requestAnimationFrame(function () { requestAnimationFrame(function () { setTimeout(resolve, 0); }); });
      setTimeout(resolve, 500);
    });
  }
  // Drawings kept in the browser by content (and theme), so a reload, a
  // visit to another page and back, or a live update shows them at once.
  function hashOf(s) {
    var a = 0x811c9dc5, b = 5381;
    for (var i = 0; i < s.length; i++) {
      var c = s.charCodeAt(i);
      a = Math.imul(a ^ c, 0x01000193);
      b = (Math.imul(b, 33) + c) | 0;
    }
    return (a >>> 0).toString(36) + (b >>> 0).toString(36) + s.length.toString(36);
  }
  function stored(key) {
    try { return localStorage.getItem("mdv-draw:" + hashOf(key)); } catch (e) { return null; }
  }
  function store(key, html) {
    if (html.length > 500000) return;
    var k = "mdv-draw:" + hashOf(key);
    try {
      localStorage.setItem(k, html);
    } catch (e) {
      // Full: start over (only our drawings are dropped).
      try {
        Object.keys(localStorage).forEach(function (n) { if (n.indexOf("mdv-draw:") === 0) localStorage.removeItem(n); });
        localStorage.setItem(k, html);
      } catch (e2) {}
    }
  }
  // On screen now (blocks in detached content never are).
  function onScreen(el) {
    var r = el.getBoundingClientRect();
    return r.height > 0 && r.bottom > 0 && r.top < window.innerHeight;
  }
  function pause() { return new Promise(function (r) { setTimeout(r, 0); }); }

  // Each drawn diagram's source, so it can be redrawn for another theme.
  var sources = new WeakMap();
  var seq = 0;
  var loading = null;
  var configured = null;

  function theme() {
    var t = document.documentElement.getAttribute("data-mdv-theme");
    var dark = t === "dark" ||
      (t === "auto" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    return dark ? "dark" : "default";
  }

  // Standalone pages inline the library; served pages fetch it on first use.
  function load() {
    // Check for the library itself: an element with id="mermaid" (e.g. a
    // "Mermaid" heading) also shows up as window.mermaid.
    if (window.mermaid && typeof window.mermaid.initialize === "function") return Promise.resolve();
    if (!src) return Promise.reject(new Error("mermaid.js not available"));
    if (!loading) {
      loading = new Promise(function (resolve, reject) {
        var s = document.createElement("script");
        s.src = src;
        s.nonce = nonce;
        s.onload = resolve;
        s.onerror = function () { loading = null; reject(new Error("failed to load " + src)); };
        document.head.appendChild(s);
      });
    }
    return loading;
  }

  function keyFor(t, pre) { return "mermaid " + version + " " + t + "\n" + pre.textContent; }
  function place(pre, svg, code) {
    var div = document.createElement("div");
    div.className = "mermaid";
    div.innerHTML = svg;
    // Keep the source line, for click-to-source and reading positions.
    if (pre.hasAttribute("data-sourcepos")) div.setAttribute("data-sourcepos", pre.getAttribute("data-sourcepos"));
    sources.set(div, code);
    pre.replaceWith(div);
  }

  // Before the first paint: diagrams this browser has drawn before go in
  // at once, so the page doesn't move when they arrive. (Fetching the
  // library early for the others, with a preload, made them slower.)
  function drawRemembered(root) {
    var t = theme();
    Array.prototype.forEach.call(root.querySelectorAll("pre.mermaid"), function (pre) {
      var key = keyFor(t, pre);
      var svg = cache.get(key) || stored(key);
      if (!svg) return;
      cache.set(key, svg);
      place(pre, svg, pre.textContent);
    });
  }

  async function render(root) {
    var blocks = Array.prototype.slice.call(root.querySelectorAll("pre.mermaid"));
    if (!blocks.length) return;
    var t = theme();
    var keyOf = function (pre) { return keyFor(t, pre); };
    var known = function (pre) {
      var key = keyOf(pre);
      if (!cache.has(key)) {
        var s = stored(key);
        if (s) cache.set(key, s);
      }
      return cache.has(key);
    };
    // Drawings already known first (no library needed), then the ones on
    // screen, then the rest, letting the page breathe between them.
    var rank = function (pre) { return known(pre) ? 0 : onScreen(pre) ? 1 : 2; };
    blocks = blocks.map(function (b, i) { return [rank(b), i, b]; })
      .sort(function (a, b) { return a[0] - b[0] || a[1] - b[1]; })
      .map(function (x) { return x[2]; });
    for (var i = 0; i < blocks.length; i++) {
      var pre = blocks[i];
      if (!pre.isConnected && root.isConnected) continue;
      var code = pre.textContent;
      var key = keyOf(pre);
      var svg = cache.get(key);
      if (!svg) {
        await load();
        if (configured !== t) {
          window.mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: t });
          configured = t;
        }
        if (i > 0) await pause();
        var id = "mdv-mermaid-" + ++seq;
        try {
          svg = (await window.mermaid.render(id, code)).svg;
          cache.set(key, svg);
          store(key, svg);
        } catch (e) {
          var stray = document.getElementById("d" + id);
          if (stray) stray.remove();
          pre.classList.add("mermaid-error");
          pre.title = "Mermaid: " + ((e && e.message) || e);
          continue;
        }
      }
      place(pre, svg, code);
    }
  }

  // Redraw every diagram in the current theme, off-screen first so the
  // page doesn't flicker.
  async function retheme() {
    var body = document.querySelector(".markdown-body");
    var divs = Array.prototype.filter.call(body.querySelectorAll("div.mermaid"), function (d) {
      return sources.has(d);
    });
    if (!divs.length) return;
    var staging = document.createElement("div");
    divs.forEach(function (d) {
      var pre = document.createElement("pre");
      pre.className = "mermaid";
      pre.textContent = sources.get(d);
      if (d.hasAttribute("data-sourcepos")) pre.setAttribute("data-sourcepos", d.getAttribute("data-sourcepos"));
      staging.appendChild(pre);
    });
    await render(staging);
    var drawn = Array.prototype.slice.call(staging.children);
    divs.forEach(function (d, i) { if (drawn[i]) d.replaceWith(drawn[i]); });
  }
  (window.mdvRetheme = window.mdvRetheme || []).push(retheme);

  (window.mdvEnhancers = window.mdvEnhancers || []).push(render);
  try { drawRemembered(document.querySelector(".markdown-body")); } catch (e) { console.error("markdown-viewer:", e); }
  // live.js waits for these before comparing updates with the page.
  (window.mdvInitial = window.mdvInitial || []).push(afterPaint().then(function () {
    return render(document.querySelector(".markdown-body"));
  }).catch(function (e) {
    console.error("markdown-viewer:", e);
  }));
})();
