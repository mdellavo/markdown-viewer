// markdown-viewer live reload. When the source changes, the server sends
// the new HTML; this replaces only the top-level blocks that changed
// (matching unchanged blocks from both ends), draws diagrams and math for
// just those, and tells the UI which block changed first (`mdv:updated`).
// It also follows `--goto` requests, and when the server stops it closes the
// window (if markdown-viewer opened it) or says so.
(function () {
  var script = document.currentScript;
  var closeOnExit = script && script.dataset.closeOnExit === "true";
  var doc = (script && script.dataset.doc) || "";
  var body = document.querySelector(".markdown-body");
  var latest = 0;
  // Comparison keys for the blocks on the page, as last sent by the server
  // (before diagrams and math were drawn). null until the first update.
  var lastKeys = null;
  var events = new EventSource("/__mdv/events?doc=" + encodeURIComponent(doc));

  // A block's markup without source positions, which shift whenever lines
  // are inserted or removed above it.
  function key(el) {
    return el.outerHTML.replace(/ data-sourcepos="[^"]*"/g, "");
  }

  function parse(html) {
    var tpl = document.createElement("template");
    tpl.innerHTML = html;
    return Array.prototype.slice.call(tpl.content.children);
  }

  // Draw diagrams and math (and add copy buttons) in new content.
  function enhance(container) {
    return Promise.all((window.mdvEnhancers || []).map(function (render) {
      return render(container);
    }));
  }

  // Folding a callout is the reader's choice, not a change to the
  // document: keep it across updates (see ui.js setFold).
  function saveFolds(container) {
    var saved = [];
    container.querySelectorAll(".callout").forEach(function (c, i) {
      var orig = c.getAttribute("data-mdv-fold-orig");
      if (orig === null) return;
      saved.push([i, c.getAttribute("data-callout-fold")]);
      c.setAttribute("data-callout-fold", orig);
      c.removeAttribute("data-mdv-fold-orig");
    });
    return saved;
  }
  function restoreFolds(container, saved) {
    var callouts = container.querySelectorAll(".callout");
    saved.forEach(function (s) {
      var c = callouts[s[0]];
      if (!c) return;
      c.setAttribute("data-mdv-fold-orig", c.getAttribute("data-callout-fold"));
      c.setAttribute("data-callout-fold", s[1]);
    });
  }

  // Folded parts of a [TOC]: the reader's choice, kept across updates.
  // Each fold is known by the heading it's for ("" for the whole list).
  function tocKey(details) {
    var link = details.querySelector(":scope > summary > a");
    return link ? link.getAttribute("href") : "";
  }
  function saveTocFolds(container) {
    var closed = [];
    container.querySelectorAll(".mdv-toc details:not([open])").forEach(function (d) {
      closed.push(tocKey(d));
      d.open = true;
    });
    return closed;
  }
  function restoreTocFolds(container, closed) {
    if (!closed.length) return;
    container.querySelectorAll(".mdv-toc details").forEach(function (d) {
      if (closed.indexOf(tocKey(d)) !== -1) d.open = false;
    });
  }

  // Sorted tables (ui.js): back to the file's order around updates, so
  // comparisons and source positions match the server's markup.
  function saveSorts(container) {
    return window.mdvSort ? window.mdvSort.save(container) : [];
  }
  function restoreSorts(container, saved) {
    if (window.mdvSort && saved.length) window.mdvSort.restore(container, saved);
  }

  // Undo transient UI decoration so it doesn't count as a change.
  function clean(container) {
    container.querySelectorAll(".mdv-flash, [class*='mdv-git-']").forEach(function (n) {
      n.classList.remove("mdv-flash", "mdv-git-added", "mdv-git-modified", "mdv-git-deleted");
      if (!n.getAttribute("class")) n.removeAttribute("class");
    });
  }

  // Copy source positions from fresh markup onto an unchanged block (and
  // its descendants), so click-to-source stays accurate below an edit.
  function syncPositions(onPage, fresh) {
    var a = [onPage].concat(Array.prototype.slice.call(onPage.querySelectorAll("[data-sourcepos]")))
      .filter(function (n) { return n.hasAttribute("data-sourcepos"); });
    var b = [fresh].concat(Array.prototype.slice.call(fresh.querySelectorAll("[data-sourcepos]")))
      .filter(function (n) { return n.hasAttribute("data-sourcepos"); });
    if (a.length !== b.length) return;
    for (var i = 0; i < a.length; i++) a[i].setAttribute("data-sourcepos", b[i].getAttribute("data-sourcepos"));
  }

  // The first update after connecting: the page already shows this content
  // (unless the file changed in between), so just remember it, replacing
  // the page only if it differs, and never jump to a "change".
  function firstSync(blocks, version) {
    var next = document.createElement("div");
    blocks.forEach(function (b) { next.appendChild(b.cloneNode(true)); });
    // Wait for the page's own first render (diagrams, math) too, or a
    // half-drawn page would compare as changed.
    return Promise.all([enhance(next), Promise.all(window.mdvInitial || [])]).then(function () {
      if (version !== latest) return;
      lastKeys = blocks.map(key);
      var folds = saveFolds(body);
      var sorts = saveSorts(body);
      var tocFolds = saveTocFolds(body);
      clean(body);
      var same = body.children.length === next.children.length &&
        Array.prototype.every.call(body.children, function (c, i) { return c.isEqualNode(next.children[i]); });
      if (!same) body.replaceChildren.apply(body, Array.prototype.slice.call(next.childNodes));
      restoreFolds(body, folds);
      restoreSorts(body, sorts);
      restoreTocFolds(body, tocFolds);
      document.dispatchEvent(new CustomEvent("mdv:updated", { detail: { changed: null } }));
    });
  }

  function update(blocks, version) {
    var keys = blocks.map(key);
    var oldN = lastKeys.length, newN = keys.length;
    var head = 0;
    while (head < oldN && head < newN && lastKeys[head] === keys[head]) head++;
    var tail = 0;
    while (tail < oldN - head && tail < newN - head && lastKeys[oldN - 1 - tail] === keys[newN - 1 - tail]) tail++;
    var fresh = document.createElement("div");
    blocks.slice(head, newN - tail).forEach(function (b) { fresh.appendChild(b); });
    return enhance(fresh).then(function () {
      if (version !== latest) return;
      if (body.children.length !== oldN) {
        // The page and our record disagree (shouldn't happen): resync.
        lastKeys = null;
        return firstSync(blocks, version);
      }
      var folds = saveFolds(body);
      var sorts = saveSorts(body);
      var tocFolds = saveTocFolds(body);
      var x = window.scrollX, y = window.scrollY;
      var after = body.children[oldN - tail] || null;
      for (var i = oldN - tail - 1; i >= head; i--) body.children[i].remove();
      Array.prototype.slice.call(fresh.children).forEach(function (n) { body.insertBefore(n, after); });
      // Unchanged blocks below the edit: refresh their source positions.
      for (var t = 0; t < tail; t++) {
        syncPositions(body.children[newN - tail + t], blocks[newN - tail + t]);
      }
      restoreFolds(body, folds);
      restoreSorts(body, sorts);
      restoreTocFolds(body, tocFolds);
      window.scrollTo(x, y);
      lastKeys = keys;
      var moved = head === oldN && head === newN;
      document.dispatchEvent(new CustomEvent("mdv:updated", {
        detail: { changed: moved ? null : body.children[head] || body.lastElementChild },
      }));
    });
  }

  events.addEventListener("update", function (e) {
    var version = ++latest;
    var blocks = parse(e.data);
    var run = lastKeys === null ? firstSync(blocks, version) : update(blocks, version);
    run.catch(function (err) { console.error("markdown-viewer:", err); });
  });

  // `markdown-viewer --goto file:line`: show that line, opening the
  // document first if this page shows another one.
  events.addEventListener("goto", function (e) {
    var msg = JSON.parse(e.data);
    if (msg.doc === doc) {
      document.dispatchEvent(new CustomEvent("mdv:goto", { detail: { line: msg.line } }));
    } else {
      location.href = "/" + msg.doc.split("/").map(encodeURIComponent).join("/") + "#mdv-line-" + msg.line;
    }
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
