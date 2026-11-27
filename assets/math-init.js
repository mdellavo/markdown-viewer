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
      loading = Promise.all([
        add("link", { rel: "stylesheet", href: cssSrc }),
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

  async function render(root) {
    var nodes = root.querySelectorAll("[data-math-style]");
    if (!nodes.length) return;
    await load();
    for (var i = 0; i < nodes.length; i++) {
      var el = nodes[i];
      var display = el.getAttribute("data-math-style") === "display";
      // ```math fences arrive as <pre><code data-math-style>; replace the <pre>.
      var target = el.tagName === "CODE" && el.parentElement.tagName === "PRE"
        ? el.parentElement : el;
      var out = document.createElement(display ? "div" : "span");
      out.className = display ? "math math-display" : "math math-inline";
      var pos = target.getAttribute("data-sourcepos") || el.getAttribute("data-sourcepos");
      if (pos) out.setAttribute("data-sourcepos", pos);
      // throwOnError: false shows bad TeX in red instead of failing the page;
      // KaTeX's default trust: false keeps \href, \url etc. disabled.
      window.katex.render(el.textContent.trim(), out, {
        displayMode: display,
        throwOnError: false,
      });
      target.replaceWith(out);
    }
  }

  (window.mdvEnhancers = window.mdvEnhancers || []).push(render);
  // live.js waits for these before comparing updates with the page.
  (window.mdvInitial = window.mdvInitial || []).push(afterPaint().then(function () {
    return render(document.querySelector(".markdown-body"));
  }).catch(function (e) {
    console.error("markdown-viewer:", e);
  }));
})();
