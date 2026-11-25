// markdown-viewer viewer UI: toolbar, theme switching, contents and files
// sidebar, keyboard shortcuts, copy buttons, word count, and highlighting
// the block you just edited. Lives outside .markdown-body so live updates
// never replace it.
(function () {
  "use strict";
  var script = document.currentScript;
  var served = script.dataset.served === "true";
  var currentDoc = script.dataset.doc || "";
  var root = document.documentElement;
  var body = document.querySelector(".markdown-body");

  // Icons: Primer Octicons (MIT), see assets/LICENSE-octicons.
  var ICONS = {
    "copy": ["M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25Z", "M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"],
    "check": ["M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"],
    "list-unordered": ["M5.75 2.5h8.5a.75.75 0 0 1 0 1.5h-8.5a.75.75 0 0 1 0-1.5Zm0 5h8.5a.75.75 0 0 1 0 1.5h-8.5a.75.75 0 0 1 0-1.5Zm0 5h8.5a.75.75 0 0 1 0 1.5h-8.5a.75.75 0 0 1 0-1.5ZM2 14a1 1 0 1 1 0-2 1 1 0 0 1 0 2Zm1-6a1 1 0 1 1-2 0 1 1 0 0 1 2 0ZM2 4a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"],
    "file-directory": ["M0 2.75C0 1.784.784 1 1.75 1H5c.55 0 1.07.26 1.4.7l.9 1.2a.25.25 0 0 0 .2.1h6.75c.966 0 1.75.784 1.75 1.75v8.5A1.75 1.75 0 0 1 14.25 15H1.75A1.75 1.75 0 0 1 0 13.25Zm1.75-.25a.25.25 0 0 0-.25.25v10.5c0 .138.112.25.25.25h12.5a.25.25 0 0 0 .25-.25v-8.5a.25.25 0 0 0-.25-.25H7.5c-.55 0-1.07-.26-1.4-.7l-.9-1.2a.25.25 0 0 0-.2-.1Z"],
    "sun": ["M8 12a4 4 0 1 1 0-8 4 4 0 0 1 0 8Zm0-1.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Zm5.657-8.157a.75.75 0 0 1 0 1.061l-1.061 1.06a.749.749 0 0 1-1.275-.326.749.749 0 0 1 .215-.734l1.06-1.06a.75.75 0 0 1 1.06 0Zm-9.193 9.193a.75.75 0 0 1 0 1.06l-1.06 1.061a.75.75 0 1 1-1.061-1.06l1.06-1.061a.75.75 0 0 1 1.061 0ZM8 0a.75.75 0 0 1 .75.75v1.5a.75.75 0 0 1-1.5 0V.75A.75.75 0 0 1 8 0ZM3 8a.75.75 0 0 1-.75.75H.75a.75.75 0 0 1 0-1.5h1.5A.75.75 0 0 1 3 8Zm13 0a.75.75 0 0 1-.75.75h-1.5a.75.75 0 0 1 0-1.5h1.5A.75.75 0 0 1 16 8Zm-8 5a.75.75 0 0 1 .75.75v1.5a.75.75 0 0 1-1.5 0v-1.5A.75.75 0 0 1 8 13Zm3.536-1.464a.75.75 0 0 1 1.06 0l1.061 1.06a.75.75 0 0 1-1.06 1.061l-1.061-1.06a.75.75 0 0 1 0-1.061ZM2.343 2.343a.75.75 0 0 1 1.061 0l1.06 1.061a.751.751 0 0 1-.018 1.042.751.751 0 0 1-1.042.018l-1.06-1.06a.75.75 0 0 1 0-1.06Z"],
    "moon": ["M9.598 1.591a.749.749 0 0 1 .785-.175 7.001 7.001 0 1 1-8.967 8.967.75.75 0 0 1 .961-.96 5.5 5.5 0 0 0 7.046-7.046.75.75 0 0 1 .175-.786Zm1.616 1.945a7 7 0 0 1-7.678 7.678 5.499 5.499 0 1 0 7.678-7.678Z"],
    "device-desktop": ["M14.25 1c.966 0 1.75.784 1.75 1.75v7.5A1.75 1.75 0 0 1 14.25 12h-3.727c.099 1.041.52 1.872 1.292 2.757A.752.752 0 0 1 11.25 16h-6.5a.75.75 0 0 1-.565-1.243c.772-.885 1.192-1.716 1.292-2.757H1.75A1.75 1.75 0 0 1 0 10.25v-7.5C0 1.784.784 1 1.75 1ZM1.75 2.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h12.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25ZM9.018 12H6.982a5.72 5.72 0 0 1-.765 2.5h3.566a5.72 5.72 0 0 1-.765-2.5Z"],
    "question": ["M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM6.92 6.085h.001a.749.749 0 1 1-1.342-.67c.169-.339.436-.701.849-.977C6.845 4.16 7.369 4 8 4a2.756 2.756 0 0 1 1.637.525c.503.377.863.965.863 1.725 0 .448-.115.83-.329 1.15-.205.307-.47.513-.692.662-.109.072-.22.138-.313.195l-.006.004a6.24 6.24 0 0 0-.26.16.952.952 0 0 0-.276.245.75.75 0 0 1-1.248-.832c.184-.264.42-.489.692-.661.103-.067.207-.132.313-.195l.007-.004c.1-.061.182-.11.258-.161a.969.969 0 0 0 .277-.245C8.96 6.514 9 6.427 9 6.25a.612.612 0 0 0-.262-.525A1.27 1.27 0 0 0 8 5.5c-.369 0-.595.09-.74.187a1.01 1.01 0 0 0-.34.398ZM9 11a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"],
    "x": ["M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z"],
  };
  function icon(name) {
    return '<svg viewBox="0 0 16 16" aria-hidden="true">' +
      ICONS[name].map(function (d) { return '<path d="' + d + '"></path>'; }).join("") +
      "</svg>";
  }
  function el(tag, attrs, html) {
    var e = document.createElement(tag);
    for (var k in attrs) e.setAttribute(k, attrs[k]);
    if (html != null) e.innerHTML = html;
    return e;
  }

  // ---- Preferences -------------------------------------------------------
  // Served pages save to the viewer (it picks a new port each run, so the
  // browser's own storage wouldn't persist); standalone pages use
  // localStorage.
  function loadLocal() {
    try { return JSON.parse(localStorage.getItem("mdv-prefs") || "{}"); } catch (e) { return {}; }
  }
  function savePrefs(update) {
    if (served) {
      fetch("/__mdv/prefs", {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-MDV": "1" },
        body: JSON.stringify(update),
      }).catch(function () {});
    } else {
      try {
        var prefs = loadLocal();
        for (var k in update) prefs[k] = update[k];
        localStorage.setItem("mdv-prefs", JSON.stringify(prefs));
      } catch (e) {}
    }
  }
  if (!served) {
    var local = loadLocal();
    if (local.theme && !root.hasAttribute("data-mdv-fixed-theme")) root.setAttribute("data-mdv-theme", local.theme);
    if (local.sidebar !== undefined) root.setAttribute("data-mdv-sidebar", local.sidebar || "");
  }

  // ---- Building the UI ---------------------------------------------------
  var toolbar = el("div", { "class": "mdv-ui mdv-toolbar", role: "toolbar", "aria-label": "Viewer" });
  var stats = el("span", { "class": "mdv-stats", title: "Words and reading time" });
  var contentsBtn = el("button", { type: "button", title: "Contents (o)", "aria-label": "Contents", "aria-pressed": "false" }, icon("list-unordered"));
  var filesBtn = el("button", { type: "button", title: "Files (f)", "aria-label": "Files", "aria-pressed": "false" }, icon("file-directory"));
  var themeBtn = el("button", { type: "button" });
  var helpBtn = el("button", { type: "button", title: "Keyboard shortcuts (?)", "aria-label": "Keyboard shortcuts" }, icon("question"));
  toolbar.append(stats, contentsBtn);
  if (served) toolbar.append(filesBtn);
  toolbar.append(themeBtn, helpBtn);

  var sidebar = el("nav", { "class": "mdv-ui mdv-sidebar", "aria-label": "Document" });
  var tabs = el("div", { "class": "mdv-tabs", role: "tablist" });
  var contentsTab = el("button", { type: "button", role: "tab", "aria-selected": "true" }, "Contents");
  var filesTab = el("button", { type: "button", role: "tab", "aria-selected": "false" }, "Files");
  var closeBtn = el("button", { type: "button", "class": "mdv-close", title: "Close (Esc)", "aria-label": "Close sidebar" }, icon("x"));
  tabs.append(contentsTab);
  if (served) tabs.append(filesTab);
  tabs.append(closeBtn);
  var contentsPanel = el("div", { "class": "mdv-panel", role: "tabpanel" });
  var filesPanel = el("div", { "class": "mdv-panel", role: "tabpanel", hidden: "" });
  var filter = el("input", { "class": "mdv-filter", type: "search", placeholder: "Filter files…", "aria-label": "Filter files" });
  var fileList = el("div");
  filesPanel.append(filter, fileList);
  sidebar.append(tabs, contentsPanel, filesPanel);

  var KEYS = [
    ["t", "Switch theme: auto, light, dark"],
    ["o", "Show or hide the contents"],
    served ? ["f", "Find a file"] : null,
    ["j / k", "Scroll down / up"],
    ["d / u", "Half a page down / up"],
    ["g / G", "Top / bottom"],
    ["n / p", "Next / previous heading"],
    ["?", "Show this help"],
    ["Esc", "Close"],
  ].filter(Boolean);
  var overlay = el("div", { "class": "mdv-ui mdv-overlay", hidden: "" });
  overlay.append(el("div", { "class": "mdv-help", role: "dialog", "aria-label": "Keyboard shortcuts" },
    "<h2>Keyboard shortcuts</h2><table>" + KEYS.map(function (k) {
      return "<tr><td>" + k[0].split(" / ").map(function (x) { return "<kbd>" + x + "</kbd>"; }).join(" / ") +
        "</td><td>" + k[1] + "</td></tr>";
    }).join("") + "</table>"));
  var toast = el("div", { "class": "mdv-ui mdv-toast", role: "status" });
  document.body.append(toolbar, sidebar, overlay, toast);

  var toastTimer;
  function showToast(text) {
    toast.textContent = text;
    toast.classList.add("mdv-show");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () { toast.classList.remove("mdv-show"); }, 1400);
  }

  // ---- Theme -------------------------------------------------------------
  var THEMES = ["auto", "light", "dark"];
  var THEME_ICONS = { auto: "device-desktop", light: "sun", dark: "moon" };
  function theme() { return root.getAttribute("data-mdv-theme") || "auto"; }
  function updateThemeButton() {
    var t = theme();
    themeBtn.innerHTML = icon(THEME_ICONS[t]);
    themeBtn.title = "Theme: " + t + " (t)";
    themeBtn.setAttribute("aria-label", "Theme: " + t);
  }
  function retheme() {
    (window.mdvRetheme || []).forEach(function (f) {
      Promise.resolve().then(f).catch(function (e) { console.error("markdown-viewer:", e); });
    });
  }
  function cycleTheme() {
    var next = THEMES[(THEMES.indexOf(theme()) + 1) % THEMES.length];
    root.setAttribute("data-mdv-theme", next);
    updateThemeButton();
    retheme();
    savePrefs({ theme: next });
    showToast("Theme: " + next + (next === "auto" ? " (follows system)" : ""));
  }
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", function () {
    if (theme() === "auto") retheme();
  });
  updateThemeButton();

  // ---- Sidebar -----------------------------------------------------------
  function sidebarOpen() { return root.classList.contains("mdv-sidebar-open"); }
  function selectTab(name) {
    var files = name === "files" && served;
    contentsTab.setAttribute("aria-selected", String(!files));
    filesTab.setAttribute("aria-selected", String(files));
    contentsPanel.hidden = files;
    filesPanel.hidden = !files;
    contentsBtn.setAttribute("aria-pressed", String(sidebarOpen() && !files));
    filesBtn.setAttribute("aria-pressed", String(sidebarOpen() && files));
    if (files) loadFiles();
  }
  function openSidebar(name, focusFilter) {
    root.classList.add("mdv-sidebar-open");
    selectTab(name);
    savePrefs({ sidebar: name });
    if (name === "files" && focusFilter) setTimeout(function () { filter.focus(); filter.select(); }, 50);
    if (name === "contents") trackActive();
  }
  function closeSidebar() {
    root.classList.remove("mdv-sidebar-open");
    selectTab(contentsPanel.hidden ? "files" : "contents");
    savePrefs({ sidebar: "" });
  }
  function toggleSidebar(name, focusFilter) {
    var showing = sidebarOpen() && (name === "files" ? !filesPanel.hidden : !contentsPanel.hidden);
    if (showing) closeSidebar(); else openSidebar(name, focusFilter);
  }
  function overlaying() { return window.innerWidth < 1200; }
  contentsBtn.addEventListener("click", function () { toggleSidebar("contents"); });
  filesBtn.addEventListener("click", function () { toggleSidebar("files", true); });
  contentsTab.addEventListener("click", function () { openSidebar("contents"); });
  filesTab.addEventListener("click", function () { openSidebar("files", true); });
  closeBtn.addEventListener("click", closeSidebar);
  themeBtn.addEventListener("click", cycleTheme);
  helpBtn.addEventListener("click", function () { overlay.hidden = false; });
  overlay.addEventListener("click", function (e) { if (e.target === overlay) overlay.hidden = true; });

  // Contents: headings h1–h4, indented relative to the top level used.
  var headings = [];
  function buildContents() {
    headings = Array.prototype.filter.call(body.querySelectorAll("h1, h2, h3, h4"), function (h) { return h.id; });
    contentsPanel.textContent = "";
    if (!headings.length) {
      contentsPanel.append(el("div", { "class": "mdv-empty" }, "No headings"));
      return;
    }
    var top = Math.min.apply(null, headings.map(function (h) { return +h.tagName[1]; }));
    headings.forEach(function (h) {
      var a = el("a", { href: "#" + encodeURIComponent(h.id), "class": "mdv-l" + (+h.tagName[1] - top + 1) });
      a.textContent = h.textContent.trim();
      a.title = a.textContent;
      a.addEventListener("click", function () { if (overlaying()) closeSidebar(); });
      contentsPanel.append(a);
    });
    trackActive();
  }
  var activeLink = null;
  function trackActive() {
    if (!sidebarOpen() || contentsPanel.hidden || !headings.length) return;
    var current = 0;
    for (var i = 0; i < headings.length; i++) {
      if (headings[i].getBoundingClientRect().top <= 90) current = i; else break;
    }
    var link = contentsPanel.querySelectorAll("a")[current];
    if (link === activeLink) return;
    if (activeLink) activeLink.classList.remove("mdv-active");
    activeLink = link;
    if (!link) return;
    link.classList.add("mdv-active");
    var p = contentsPanel.getBoundingClientRect(), r = link.getBoundingClientRect();
    if (r.top < p.top || r.bottom > p.bottom) link.scrollIntoView({ block: "nearest" });
  }
  var ticking = false;
  window.addEventListener("scroll", function () {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(function () { ticking = false; trackActive(); });
  }, { passive: true });

  // Files: every Markdown file under the served folder.
  var files = null;
  function loadFiles() {
    if (files) return;
    files = [];
    fileList.append(el("div", { "class": "mdv-empty" }, "Loading…"));
    fetch("/__mdv/files").then(function (r) { return r.json(); }).then(function (data) {
      files = data.files || [];
      filesLoaded = true;
      renderFiles();
      if (data.truncated) fileList.append(el("div", { "class": "mdv-empty" }, "Showing the first " + files.length + " files"));
    }).catch(function (err) {
      console.error("markdown-viewer:", err);
      fileList.textContent = "";
      fileList.append(el("div", { "class": "mdv-empty" }, "Couldn't list files"));
    });
  }
  function renderFiles() {
    var q = filter.value.trim().toLowerCase();
    fileList.textContent = "";
    var shown = files.filter(function (f) { return !q || f.toLowerCase().indexOf(q) !== -1; });
    if (!shown.length) {
      fileList.append(el("div", { "class": "mdv-empty" }, q ? "No matching files" : "No Markdown files"));
      return;
    }
    var lastDir = null;
    shown.forEach(function (f) {
      var slash = f.lastIndexOf("/");
      var dir = slash === -1 ? "" : f.slice(0, slash);
      if (dir !== lastDir) {
        if (dir) {
          var heading = el("div", { "class": "mdv-dir" });
          heading.textContent = dir + "/";
          fileList.append(heading);
        }
        lastDir = dir;
      }
      var a = el("a", { href: "/" + f.split("/").map(encodeURIComponent).join("/"), title: f });
      a.textContent = f.slice(slash + 1);
      if (f === currentDoc) a.classList.add("mdv-active");
      fileList.append(a);
    });
  }
  var filesLoaded = false;
  filter.addEventListener("input", function () { if (filesLoaded) renderFiles(); });
  filter.addEventListener("keydown", function (e) {
    if (e.key === "Enter") {
      var first = fileList.querySelector("a");
      if (first) location.href = first.href;
    }
  });

  // Restore the sidebar the way it was last left.
  var initialSidebar = root.getAttribute("data-mdv-sidebar");
  if (initialSidebar === "files" && !served) initialSidebar = "contents";
  if (initialSidebar === "contents" || initialSidebar === "files") {
    root.classList.add("mdv-sidebar-open");
    selectTab(initialSidebar);
  } else {
    selectTab("contents");
  }

  // ---- Copy buttons on code blocks ---------------------------------------
  function addCopyButtons(container) {
    container.querySelectorAll("pre").forEach(function (pre) {
      if (pre.classList.contains("mermaid") || pre.hasAttribute("data-math-style")) return;
      if (pre.parentElement && pre.parentElement.classList.contains("mdv-code")) return;
      var wrap = el("div", { "class": "mdv-code" });
      pre.parentNode.insertBefore(wrap, pre);
      wrap.append(pre, el("button", {
        type: "button", "class": "mdv-copy mdv-ui", title: "Copy", "aria-label": "Copy code",
      }, icon("copy")));
    });
  }
  document.addEventListener("click", function (e) {
    var button = e.target.closest && e.target.closest(".mdv-copy");
    if (!button) return;
    var text = button.parentElement.querySelector("pre").innerText;
    var done = function () {
      button.innerHTML = icon("check");
      button.classList.add("mdv-copied");
      setTimeout(function () { button.innerHTML = icon("copy"); button.classList.remove("mdv-copied"); }, 1500);
    };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, function () { fallbackCopy(text) && done(); });
    } else if (fallbackCopy(text)) {
      done();
    }
  });
  function fallbackCopy(text) {
    var ta = el("textarea", { style: "position:fixed;opacity:0" });
    ta.value = text;
    document.body.append(ta);
    ta.select();
    var ok = false;
    try { ok = document.execCommand("copy"); } catch (e) {}
    ta.remove();
    return ok;
  }
  (window.mdvEnhancers = window.mdvEnhancers || []).push(addCopyButtons);

  // ---- Word count --------------------------------------------------------
  function updateStats() {
    var copy = body.cloneNode(true);
    copy.querySelectorAll("pre, div.mermaid, .math, .katex, .footnote-backref, .mdv-ui, svg").forEach(function (n) { n.remove(); });
    var words = (copy.textContent.match(/[\p{L}\p{N}][\p{L}\p{N}'’_-]*/gu) || []).length;
    var minutes = Math.max(1, Math.round(words / 230));
    stats.textContent = words.toLocaleString() + " words · " + minutes + " min read";
  }

  // ---- Keyboard ----------------------------------------------------------
  function headingTops() {
    return headings.map(function (h) { return h.getBoundingClientRect().top; });
  }
  function jumpHeading(dir) {
    var tops = headingTops();
    var target = null;
    if (dir > 0) {
      for (var i = 0; i < tops.length; i++) if (tops[i] > 12) { target = headings[i]; break; }
    } else {
      for (var j = tops.length - 1; j >= 0; j--) if (tops[j] < -12) { target = headings[j]; break; }
    }
    if (target) target.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  document.addEventListener("keydown", function (e) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    var t = e.target;
    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) {
      if (e.key === "Escape") t.blur();
      return;
    }
    var page = window.innerHeight;
    // "?" arrives as Shift+"/" on some layouts and from some tools.
    var key = e.key === "/" && e.shiftKey ? "?" : e.key;
    switch (key) {
      case "t": cycleTheme(); break;
      case "o": toggleSidebar("contents"); break;
      // "Find a file" always opens the list and focuses the filter.
      case "f": if (!served) return; openSidebar("files", true); break;
      case "j": window.scrollBy(0, 80); break;
      case "k": window.scrollBy(0, -80); break;
      case "d": window.scrollBy({ top: page / 2, behavior: "smooth" }); break;
      case "u": window.scrollBy({ top: -page / 2, behavior: "smooth" }); break;
      case "g": window.scrollTo({ top: 0, behavior: "smooth" }); break;
      case "G": window.scrollTo({ top: document.documentElement.scrollHeight, behavior: "smooth" }); break;
      case "n": jumpHeading(1); break;
      case "p": jumpHeading(-1); break;
      case "?": overlay.hidden = !overlay.hidden; break;
      case "Escape":
        if (!overlay.hidden) overlay.hidden = true;
        else if (sidebarOpen() && overlaying()) closeSidebar();
        else return;
        break;
      default: return;
    }
    e.preventDefault();
  });

  // ---- Live updates: refresh, and point at what changed -----------------
  document.addEventListener("mdv:updated", function (e) {
    buildContents();
    updateStats();
    var changed = e.detail && e.detail.changed;
    if (!changed) return;
    var r = changed.getBoundingClientRect();
    if (r.bottom < 0 || r.top > window.innerHeight) changed.scrollIntoView({ block: "center" });
    changed.classList.add("mdv-flash");
    setTimeout(function () { changed.classList.remove("mdv-flash"); }, 1700);
  });

  addCopyButtons(body);
  buildContents();
  updateStats();
})();
