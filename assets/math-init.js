// markdown-viewer: typesets math ($…$, $$…$$, $`…`$ and ```math blocks,
// all marked with data-math-style by the renderer) with the bundled KaTeX.
// Registers itself in window.mdvEnhancers, which live.js runs on updated
// content before swapping it in.
(function () {
  var script = document.currentScript;
  var nonce = script && script.nonce;
  var jsSrc = script && script.dataset.katexJs;
  var cssSrc = script && script.dataset.katexCss;
  // Against the page's own address, never a <base> in the document.
  if (jsSrc) jsSrc = new URL(jsSrc, location.href).href;
  if (cssSrc) cssSrc = new URL(cssSrc, location.href).href;
  var loading = null;
  // Start once the page has been drawn, so its text shows before a library
  // loads and runs (at most 500 ms later, e.g. in a background tab).
  function afterPaint() {
    return new Promise(function (resolve) {
      // A frame's callback runs before it's drawn; the next one, after.
      requestAnimationFrame(function () { requestAnimationFrame(function () { setTimeout(resolve, 0); }); });
      setTimeout(resolve, 500);
    });
  }

  // Standalone pages inline KaTeX; served pages fetch it on first use.
  function load() {
    // Check for the library itself: an element with id="katex" (e.g. a
    // "KaTeX" heading) also shows up as window.katex.
    if (window.katex && typeof window.katex.render === "function") return Promise.resolve();
    if (!jsSrc) return Promise.reject(new Error("KaTeX not available"));
    if (!loading) {
      // The page links the styles itself when it has math to begin with.
      var linked = Array.prototype.some.call(document.querySelectorAll("link[rel=stylesheet]"), function (l) {
        return l.href === cssSrc;
      });
      loading = Promise.all([
        linked ? null : add("link", { rel: "stylesheet", href: cssSrc }),
        add("script", { src: jsSrc, nonce: nonce }),
      ]).catch(function (e) { loading = null; throw e; });
    }
    return loading;
  }

  function add(tag, attrs) {
    return new Promise(function (resolve, reject) {
      var el = document.createElement(tag);
      for (var k in attrs) el[k] = attrs[k];
      el.onload = resolve;
      el.onerror = function () { reject(new Error("failed to load " + (attrs.src || attrs.href))); };
      document.head.appendChild(el);
    });
  }

  // Typeset math kept in the browser, one entry per page (its formulas
  // only, so it doesn't grow), so a reload or a visit back shows it in the
  // first paint and lines don't shift as KaTeX runs. Keyed by KaTeX's
  // version, which is in the script's address.
  var katexVersion = jsSrc ? jsSrc.split("?v=")[1] || "" : (window.katex && window.katex.version) || "inline";
  var memoryKey = "mdv-math:" + katexVersion + ":" + location.pathname;
  var remembered = {};
  try { remembered = JSON.parse(localStorage.getItem(memoryKey) || "{}") || {}; } catch (e) { remembered = {}; }
  var used = {};
  var saveTimer = null;
  function remember() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(function () {
      // Only what's on the page now, not every version typed while editing.
      var now = {};
      document.querySelectorAll(".markdown-body .math[data-mdv-tex]").forEach(function (m) {
        var k = m.getAttribute("data-mdv-tex");
        if (used[k] != null) now[k] = used[k];
      });
      used = now;
      var json = JSON.stringify(used);
      if (json.length > 1500000) return;
      try {
        localStorage.setItem(memoryKey, json);
      } catch (e) {
        // Full: drop the other pages' math and try again.
        try {
          Object.keys(localStorage).forEach(function (n) {
            if (n.indexOf("mdv-math:") === 0 && n !== memoryKey) localStorage.removeItem(n);
          });
          localStorage.setItem(memoryKey, json);
        } catch (e2) {}
      }
    }, 300);
  }

  function parts(el) {
    var display = el.getAttribute("data-math-style") === "display";
    // ```math fences arrive as <pre><code data-math-style>; replace the <pre>.
    var target = el.tagName === "CODE" && el.parentElement.tagName === "PRE"
      ? el.parentElement : el;
    var tex = el.textContent.trim();
    return { display: display, target: target, tex: tex, key: (display ? "D" : "I") + tex };
  }
  function place(el, p, html) {
    var out = document.createElement(p.display ? "div" : "span");
    out.className = p.display ? "math math-display" : "math math-inline";
    var pos = p.target.getAttribute("data-sourcepos") || el.getAttribute("data-sourcepos");
    if (pos) out.setAttribute("data-sourcepos", pos);
    out.setAttribute("data-mdv-tex", p.key);
    out.innerHTML = html;
    p.target.replaceWith(out);
  }

  // Before the first paint: the math this page showed last time.
  function drawRemembered(root) {
    Array.prototype.forEach.call(root.querySelectorAll("[data-math-style]"), function (el) {
      var p = parts(el);
      var html = remembered[p.key];
      if (html == null) return;
      used[p.key] = html;
      place(el, p, html);
    });
  }

  async function render(root) {
    var nodes = root.querySelectorAll("[data-math-style]");
    if (!nodes.length) return;
    var fresh = false;
    for (var i = 0; i < nodes.length; i++) {
      var el = nodes[i];
      var p = parts(el);
      var html = used[p.key] != null ? used[p.key] : remembered[p.key];
      if (html == null) {
        await load();
        // throwOnError: false shows bad TeX in red instead of failing the
        // page; KaTeX's default trust: false keeps \href, \url etc. disabled.
        html = window.katex.renderToString(p.tex, { displayMode: p.display, throwOnError: false });
      }
      if (used[p.key] !== html) fresh = true;
      used[p.key] = html;
      place(el, p, html);
    }
    if (fresh) remember();
  }

  try { drawRemembered(document.querySelector(".markdown-body")); } catch (e) { console.error("markdown-viewer:", e); }
  (window.mdvEnhancers = window.mdvEnhancers || []).push(render);
  // live.js waits for these before comparing updates with the page.
  (window.mdvInitial = window.mdvInitial || []).push(afterPaint().then(function () {
    return render(document.querySelector(".markdown-body"));
  }).catch(function (e) {
    console.error("markdown-viewer:", e);
  }));
})();
