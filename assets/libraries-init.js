// markdown-viewer: draws ```vega-lite, ```abc and ```smiles blocks with the
// bundled libraries (Vega-Lite, abcjs, SmilesDrawer), like mermaid-init.js.
// data-libs maps each library to its script's address (served pages and
// static sites); standalone pages inline the library before this script.
// Registers in window.mdvEnhancers (live.js runs those on new content
// before swapping it in) and window.mdvRetheme (redraw for a new theme).
(function () {
  var script = document.currentScript;
  var nonce = script && script.nonce;
  var srcs = {};
  try { srcs = JSON.parse((script && script.dataset.libs) || "{}"); } catch (e) { srcs = {}; }
  var loading = {};
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

  // Each drawing's source and kind, to redraw it for another theme.
  var sources = new WeakMap();

  function dark() {
    var t = document.documentElement.getAttribute("data-mdv-theme");
    return t === "dark" || (t === "auto" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  }

  // The page's colours, for libraries that need them spelled out.
  function colors() {
    var body = document.querySelector(".markdown-body") || document.body;
    var s = getComputedStyle(body);
    return {
      fg: s.color,
      muted: s.getPropertyValue("--fgColor-muted").trim() || s.color,
      border: s.getPropertyValue("--borderColor-default").trim() || "#d1d9e0",
    };
  }

  // Check for the library's functions: an element with a matching id
  // (a "Vega" heading) also shows up as a global.
  var ready = {
    vega: function () {
      return window.vega && typeof window.vega.parse === "function" &&
        window.vegaLite && typeof window.vegaLite.compile === "function" &&
        window.vegaInterpreter && window.vegaInterpreter.expressionInterpreter != null;
    },
    abcjs: function () { return window.ABCJS && typeof window.ABCJS.renderAbc === "function"; },
    "smiles-drawer": function () {
      return window.SmilesDrawer && typeof window.SmilesDrawer.parse === "function";
    },
  };

  function load(id) {
    if (ready[id]()) return Promise.resolve();
    var src = srcs[id];
    if (!src) return Promise.reject(new Error(id + " not available"));
    if (!loading[id]) {
      loading[id] = new Promise(function (resolve, reject) {
        var s = document.createElement("script");
        // Against the page's own address, never a <base> in the document.
        s.src = new URL(src, location.href).href;
        s.nonce = nonce;
        s.onload = resolve;
        s.onerror = function () { loading[id] = null; reject(new Error("failed to load " + src)); };
        document.head.appendChild(s);
      });
    }
    return loading[id];
  }

  async function vegaSvg(code) {
    var spec = JSON.parse(code);
    var c = colors();
    var config = {
      background: null,
      title: { color: c.fg, subtitleColor: c.muted },
      axis: { labelColor: c.muted, titleColor: c.fg, domainColor: c.border, tickColor: c.border, gridColor: c.border },
      legend: { labelColor: c.muted, titleColor: c.fg },
      header: { labelColor: c.muted, titleColor: c.fg },
      view: { stroke: c.border },
      style: { "guide-label": { fill: c.muted }, "guide-title": { fill: c.fg } },
    };
    var lite = !spec.marks && !spec.signals || /vega-lite/.test(spec.$schema || "");
    var vg = lite ? window.vegaLite.compile(spec, { config: config }).spec : spec;
    var runtime = window.vega.parse(vg, lite ? null : config, { ast: true });
    var view = new window.vega.View(runtime, {
      renderer: "none",
      expr: window.vegaInterpreter.expressionInterpreter,
      // Data files load relative to the page (served pages only).
      loader: window.vega.loader({ baseURL: location.href }),
    });
    try {
      return await view.toSVG();
    } finally {
      view.finalize();
    }
  }

  function abcDiv(code) {
    var div = document.createElement("div");
    // abcjs measures text while drawing, so draw in the page, off-screen
    // (in a holder: abcjs styles the element it draws into).
    var holder = document.createElement("div");
    holder.style.cssText = "position:absolute;left:-10000px;top:0;width:" +
      ((document.querySelector(".markdown-body") || document.body).clientWidth || 800) + "px";
    holder.appendChild(div);
    document.body.appendChild(holder);
    try {
      window.ABCJS.renderAbc(div, code, { foregroundColor: "currentColor", add_classes: true });
    } finally {
      holder.remove();
    }
    if (!div.querySelector("svg")) throw new Error("nothing to draw");
    return div;
  }

  function smilesSvg(code) {
    return new Promise(function (resolve, reject) {
      var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      var drawer = new window.SmilesDrawer.SvgDrawer({ width: 360, height: 260, padding: 12 });
      window.SmilesDrawer.parse(code.trim(), function (tree) {
        try {
          drawer.draw(tree, svg, dark() ? "dark" : "light", false);
          resolve(svg);
        } catch (e) { reject(e); }
      }, function (e) { reject(e); });
    });
  }

  var kinds = [
    { id: "vega", cls: "mdv-lib-vega", draw: vegaSvg },
    { id: "abcjs", cls: "mdv-lib-abc", draw: abcDiv },
    { id: "smiles-drawer", cls: "mdv-lib-smiles", draw: smilesSvg },
  ];

  function keyOf(kind, code) {
    return kind.id + " " + (srcs[kind.id] || "inline") + " " + (dark() ? "dark" : "light") + "\n" + code;
  }

  async function drawBlock(kind, pre) {
    var code = pre.textContent;
    var fig = document.createElement("figure");
    fig.className = "mdv-diagram " + kind.cls.replace("mdv-lib-", "mdv-");
    var key = keyOf(kind, code);
    try {
      var cached = stored(key);
      if (cached) {
        fig.innerHTML = cached;
      } else {
        await load(kind.id);
        var out = await kind.draw(code);
        if (typeof out === "string") fig.innerHTML = out;
        else fig.appendChild(out);
        store(key, fig.innerHTML);
      }
    } catch (e) {
      pre.classList.add("mdv-lib-error");
      pre.title = ((e && e.message) || String(e));
      return;
    }
    if (pre.hasAttribute("data-sourcepos")) fig.setAttribute("data-sourcepos", pre.getAttribute("data-sourcepos"));
    sources.set(fig, { kind: kind, code: code, pre: pre.cloneNode(true) });
    pre.replaceWith(fig);
  }

  async function render(root) {
    // Every block, drawn ones first, then those on screen, then the rest.
    var all = [];
    kinds.forEach(function (kind) {
      root.querySelectorAll("pre.mdv-lib." + kind.cls).forEach(function (pre) {
        var rank = stored(keyOf(kind, pre.textContent)) ? 0 : onScreen(pre) ? 1 : 2;
        all.push([rank, all.length, kind, pre]);
      });
    });
    all.sort(function (a, b) { return a[0] - b[0] || a[1] - b[1]; });
    for (var i = 0; i < all.length; i++) {
      if (all[i][0] > 0 && i > 0) await pause();
      try {
        await drawBlock(all[i][2], all[i][3]);
      } catch (e) {
        console.error("markdown-viewer:", e);
      }
    }
  }

  // Redraw for the new theme, off-screen first so the page doesn't flicker.
  async function retheme() {
    var body = document.querySelector(".markdown-body");
    if (!body) return;
    var figs = Array.prototype.filter.call(body.querySelectorAll("figure.mdv-diagram"), function (f) {
      return sources.has(f);
    });
    for (var i = 0; i < figs.length; i++) {
      var staging = document.createElement("div");
      staging.appendChild(sources.get(figs[i]).pre.cloneNode(true));
      await render(staging);
      if (staging.firstChild && staging.firstChild.tagName === "FIGURE") figs[i].replaceWith(staging.firstChild);
    }
  }
  (window.mdvRetheme = window.mdvRetheme || []).push(retheme);
  (window.mdvEnhancers = window.mdvEnhancers || []).push(render);
  (window.mdvInitial = window.mdvInitial || []).push(afterPaint().then(function () {
    return render(document.querySelector(".markdown-body"));
  }).catch(function (e) {
    console.error("markdown-viewer:", e);
  }));
})();
