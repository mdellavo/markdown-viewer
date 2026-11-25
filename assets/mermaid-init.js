// markdown-viewer: renders ```mermaid blocks with the bundled mermaid.js.
// Registers itself in window.mdvEnhancers, which live.js runs on updated
// content *before* swapping it in, so the page doesn't flicker.
(function () {
  var nonce = document.currentScript && document.currentScript.nonce;
  var src = document.currentScript && document.currentScript.dataset.mermaidSrc;
  var cache = new Map();
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
    if (window.mermaid) return Promise.resolve();
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

  async function render(root) {
    var blocks = root.querySelectorAll("pre.mermaid");
    if (!blocks.length) return;
    await load();
    var t = theme();
    if (configured !== t) {
      window.mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: t });
      configured = t;
    }
    for (var i = 0; i < blocks.length; i++) {
      var pre = blocks[i];
      var code = pre.textContent;
      var key = t + "\n" + code;
      var svg = cache.get(key);
      if (!svg) {
        var id = "mdv-mermaid-" + ++seq;
        try {
          svg = (await window.mermaid.render(id, code)).svg;
          cache.set(key, svg);
        } catch (e) {
          var stray = document.getElementById("d" + id);
          if (stray) stray.remove();
          pre.classList.add("mermaid-error");
          pre.title = "Mermaid: " + ((e && e.message) || e);
          continue;
        }
      }
      var div = document.createElement("div");
      div.className = "mermaid";
      div.innerHTML = svg;
      pre.replaceWith(div);
    }
  }

  (window.mdvEnhancers = window.mdvEnhancers || []).push(render);
  render(document.querySelector(".markdown-body")).catch(function (e) {
    console.error("markdown-viewer:", e);
  });
})();
