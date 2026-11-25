// markdown-viewer live reload: swaps in freshly rendered content when the
// source file changes, keeping the current scroll position, and tells the
// UI which block changed first (`mdv:updated`). When the server stops it
// closes the window (if markdown-viewer opened it) or says so.
(function () {
  var script = document.currentScript;
  var closeOnExit = script && script.dataset.closeOnExit === "true";
  var doc = (script && script.dataset.doc) || "";
  var body = document.querySelector(".markdown-body");
  var latest = 0;
  var events = new EventSource("/__mdv/events?doc=" + encodeURIComponent(doc));

  // Index of the first top-level block that differs, or -1 if none.
  function firstChange(oldParent, newParent) {
    var a = oldParent.children, b = newParent.children;
    var n = Math.min(a.length, b.length);
    for (var i = 0; i < n; i++) if (!a[i].isEqualNode(b[i])) return i;
    if (a.length === b.length) return -1;
    return Math.max(0, Math.min(n, b.length - 1));
  }

  // Undo transient UI decoration so it doesn't count as a change.
  function clean(container) {
    container.querySelectorAll(".mdv-flash").forEach(function (n) {
      n.classList.remove("mdv-flash");
      if (!n.getAttribute("class")) n.removeAttribute("class");
    });
  }

  events.addEventListener("update", function (e) {
    var version = ++latest;
    var next = document.createElement("div");
    next.innerHTML = e.data;
    // Draw diagrams and math off-screen first; only the newest update is applied.
    var ready = Promise.all((window.mdvEnhancers || []).map(function (render) {
      return render(next);
    }));
    ready.catch(function (err) { console.error("markdown-viewer:", err); }).then(function () {
      if (version !== latest) return;
      clean(body);
      var changedAt = firstChange(body, next);
      if (changedAt === -1) return;
      var x = window.scrollX, y = window.scrollY;
      body.replaceChildren.apply(body, Array.prototype.slice.call(next.childNodes));
      window.scrollTo(x, y);
      document.dispatchEvent(new CustomEvent("mdv:updated", {
        detail: { changed: body.children[changedAt] || null },
      }));
    });
  });

  events.addEventListener("close", function () {
    events.close();
    // Browsers only allow this for windows that haven't navigated; if it's
    // refused (or this is a tab the user owns), the banner explains instead.
    if (closeOnExit) window.close();
    setTimeout(showStopped, 150);
  });

  events.addEventListener("error", function () {
    document.title = document.title.replace(/^(⚠ )?/, "⚠ ");
  });
  events.addEventListener("open", function () {
    document.title = document.title.replace(/^⚠ /, "");
  });

  function showStopped() {
    document.title = document.title.replace(/^(⚠ )?/, "⚠ ");
    if (document.getElementById("mdv-stopped")) return;
    var bar = document.createElement("div");
    bar.id = "mdv-stopped";
    bar.className = "mdv-ui";
    bar.textContent = "markdown-viewer stopped. This page no longer updates.";
    bar.setAttribute("role", "status");
    bar.style.cssText = "position:sticky;top:0;z-index:25;padding:8px 16px;" +
      "font:14px -apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;" +
      "text-align:center;background:#fff8c5;color:#3b2300;" +
      "border-bottom:1px solid #d4a72c66";
    document.body.insertBefore(bar, document.body.firstChild);
  }
})();
