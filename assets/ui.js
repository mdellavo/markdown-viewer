// markdown-viewer viewer UI: toolbar, theme switching, contents and files
// sidebar, keyboard shortcuts, copy buttons, word count, and highlighting
// the block you just edited. Lives outside .markdown-body so live updates
// never replace it.
(function () {
  "use strict";
  var script = document.currentScript;
  var served = script.dataset.served === "true";
  // Static site pages (-o DIR for a folder): the way up to the site's root.
  // They get the Files sidebar, from the list in _mdv/files.js.
  var siteRoot = script.getAttribute("data-site-root");
  var hasFiles = served || siteRoot !== null;
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
    "screen-full": ["M1.75 10a.75.75 0 0 1 .75.75v2.5c0 .138.112.25.25.25h2.5a.75.75 0 0 1 0 1.5h-2.5A1.75 1.75 0 0 1 1 13.25v-2.5a.75.75 0 0 1 .75-.75Zm12.5 0a.75.75 0 0 1 .75.75v2.5A1.75 1.75 0 0 1 13.25 15h-2.5a.75.75 0 0 1 0-1.5h2.5a.25.25 0 0 0 .25-.25v-2.5a.75.75 0 0 1 .75-.75ZM2.75 2.5a.25.25 0 0 0-.25.25v2.5a.75.75 0 0 1-1.5 0v-2.5C1 1.784 1.784 1 2.75 1h2.5a.75.75 0 0 1 0 1.5ZM10 1.75a.75.75 0 0 1 .75-.75h2.5c.966 0 1.75.784 1.75 1.75v2.5a.75.75 0 0 1-1.5 0v-2.5a.25.25 0 0 0-.25-.25h-2.5a.75.75 0 0 1-.75-.75Z"],
    "pencil": ["M11.013 1.427a1.75 1.75 0 0 1 2.474 0l1.086 1.086a1.75 1.75 0 0 1 0 2.474l-8.61 8.61c-.21.21-.47.364-.756.445l-3.251.93a.75.75 0 0 1-.927-.928l.929-3.25c.081-.286.235-.547.445-.758l8.61-8.61Zm.176 4.823L9.75 4.81l-6.286 6.287a.253.253 0 0 0-.064.108l-.558 1.953 1.953-.558a.253.253 0 0 0 .108-.064Zm1.238-3.763a.25.25 0 0 0-.354 0L10.811 3.75l1.439 1.44 1.263-1.263a.25.25 0 0 0 0-.354Z"],
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
  var editBtn = el("button", { type: "button", title: "Edit the source here (e, or double-click)", "aria-label": "Edit source" }, icon("pencil"));
  var presentBtn = el("button", { type: "button", title: "Present as slides (s)", "aria-label": "Present" }, icon("screen-full"));
  // Click-to-source needs the server and source positions.
  // Folder listings have no file to edit.
  var canEdit = served && !!body.querySelector("[data-sourcepos]") &&
    /\.(md|markdown|mdown|mkd|mkdn)$/i.test(currentDoc);
  toolbar.append(stats, contentsBtn);
  if (hasFiles) toolbar.append(filesBtn);
  if (canEdit) toolbar.append(editBtn);
  var copyPageBtn = el("button", { type: "button", title: "Copy the page (c)", "aria-label": "Copy the page" }, icon("copy"));
  toolbar.append(copyPageBtn, presentBtn, themeBtn, helpBtn);

  var sidebar = el("nav", { "class": "mdv-ui mdv-sidebar", "aria-label": "Document" });
  var tabs = el("div", { "class": "mdv-tabs", role: "tablist" });
  var contentsTab = el("button", { type: "button", role: "tab", "aria-selected": "true" }, "Contents");
  var filesTab = el("button", { type: "button", role: "tab", "aria-selected": "false" }, "Files");
  var closeBtn = el("button", { type: "button", "class": "mdv-close", title: "Close (Esc)", "aria-label": "Close sidebar" }, icon("x"));
  tabs.append(contentsTab);
  var linksTab = el("button", { type: "button", role: "tab", "aria-selected": "false" }, "Backlinks");
  if (hasFiles) tabs.append(filesTab);
  if (served) tabs.append(linksTab);
  tabs.append(closeBtn);
  var contentsPanel = el("div", { "class": "mdv-panel", role: "tabpanel" });
  var filesPanel = el("div", { "class": "mdv-panel", role: "tabpanel", hidden: "" });
  var filterLabel = served ? "Find files or text" : "Find files";
  var filter = el("input", { "class": "mdv-filter", type: "search", placeholder: filterLabel + "…", "aria-label": filterLabel });
  var fileList = el("div");
  var hitList = el("div");
  filesPanel.append(filter, fileList, hitList);
  var linksPanel = el("div", { "class": "mdv-panel", role: "tabpanel", hidden: "" });
  sidebar.append(tabs, contentsPanel, filesPanel, linksPanel);

  var KEYS = [
    ["t", "Switch theme: auto, light, dark"],
    ["o", "Show or hide the contents"],
    hasFiles ? ["f", "Find a file"] : null,
    served ? ["/", "Search the text of every file"] : null,
    canEdit ? ["e", "Edit the source here (or double-click a paragraph)"] : null,
    ["s", "Present as slides (arrows to move, Esc to stop)"],
    ["+ / -", "Larger / smaller text (0 resets)"],
    ["c", "Copy the page (rich text, and Markdown as plain text)"],
    served ? ["b", "Show backlinks: other files linking here"] : null,
    served ? ["l", "Check this page's links"] : null,
    served ? ["D", "Show what changed since the last git commit"] : null,
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
  var currentTab = "contents";
  function selectTab(name) {
    if ((name === "files" && !hasFiles) || (name === "links" && !served) ||
        ["contents", "files", "links"].indexOf(name) === -1) name = "contents";
    currentTab = name;
    var panels = { contents: [contentsPanel, contentsTab], files: [filesPanel, filesTab], links: [linksPanel, linksTab] };
    for (var n in panels) {
      panels[n][0].hidden = n !== name;
      panels[n][1].setAttribute("aria-selected", String(n === name));
    }
    contentsBtn.setAttribute("aria-pressed", String(sidebarOpen() && name === "contents"));
    filesBtn.setAttribute("aria-pressed", String(sidebarOpen() && name === "files"));
    if (name === "files") loadFiles();
    if (name === "links") loadBacklinks();
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
    selectTab(currentTab);
    savePrefs({ sidebar: "" });
  }
  function toggleSidebar(name, focusFilter) {
    var showing = sidebarOpen() && currentTab === name;
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
    // Mirror the CSS numbering (data-mdv-number="h1" or "h2"), four levels.
    var numberFrom = +(root.getAttribute("data-mdv-number") || "h0").slice(1);
    var counts = [0, 0, 0, 0];
    headings.forEach(function (h) {
      var level = +h.tagName[1];
      var a = el("a", { href: "#" + encodeURIComponent(h.id), "class": "mdv-l" + (level - top + 1) });
      var number = "";
      var depth = level - numberFrom;
      if (numberFrom && depth >= 0 && depth < 4) {
        counts[depth]++;
        for (var c = depth + 1; c < 4; c++) counts[c] = 0;
        number = counts.slice(0, depth + 1).join(".") + ". ";
      }
      a.textContent = number + h.textContent.trim();
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
    if (siteRoot !== null) {
      files = window.mdvSiteFiles || [];
      filesLoaded = true;
      renderFiles();
      return;
    }
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
      var a = el("a", { href: fileHref(f), title: f });
      a.textContent = f.slice(slash + 1);
      if (f === currentDoc) a.classList.add("mdv-active");
      fileList.append(a);
    });
  }
  function fileHref(f) {
    if (siteRoot === null) return "/" + f.split("/").map(encodeURIComponent).join("/");
    var page = /\.(md|markdown|mdown|mkd|mkdn)$/i.test(f)
      ? f.replace(/\.[^.]+$/, ".html") : f + ".html";
    return siteRoot + page.split("/").map(encodeURIComponent).join("/");
  }
  var filesLoaded = false;
  var searchTimer, searchSeq = 0;
  filter.addEventListener("input", function () {
    if (filesLoaded) renderFiles();
    clearTimeout(searchTimer);
    searchTimer = setTimeout(searchText, 250);
  });
  filter.addEventListener("keydown", function (e) {
    if (e.key === "Enter") {
      var first = fileList.querySelector("a") || hitList.querySelector("a");
      if (first) location.href = first.href;
    }
  });

  // Text matches across every file, below the file-name matches.
  function searchText() {
    if (!served) return;
    var q = filter.value.trim();
    var seq = ++searchSeq;
    if (q.length < 2) { hitList.textContent = ""; return; }
    fetch("/__mdv/search?q=" + encodeURIComponent(q)).then(function (r) { return r.json(); }).then(function (data) {
      if (seq !== searchSeq) return;
      hitList.textContent = "";
      var hits = data.hits || [];
      var heading = el("div", { "class": "mdv-dir" });
      heading.textContent = hits.length ? "In files" : "No text matches";
      hitList.append(heading);
      hits.forEach(function (h) {
        var a = el("a", {
          "class": "mdv-hit",
          href: "/" + h.file.split("/").map(encodeURIComponent).join("/") + "#mdv-line-" + h.line,
          title: h.file + ":" + h.line,
        });
        var where = el("span", { "class": "mdv-hit-file" });
        where.textContent = h.file + ":" + h.line;
        a.append(where, highlighted(h.text, q));
        hitList.append(a);
      });
    }).catch(function (err) { console.error("markdown-viewer:", err); });
  }

  // The snippet with each occurrence of the query marked (built as DOM
  // nodes, never as HTML, so file contents can't inject markup).
  function highlighted(text, q) {
    var span = el("span", { "class": "mdv-hit-text" });
    var lower = text.toLowerCase(), needle = q.toLowerCase(), at = 0;
    while (true) {
      var i = lower.indexOf(needle, at);
      if (i === -1) break;
      span.append(document.createTextNode(text.slice(at, i)));
      var mark = document.createElement("mark");
      mark.textContent = text.slice(i, i + needle.length);
      span.append(mark);
      at = i + needle.length;
    }
    span.append(document.createTextNode(text.slice(at)));
    return span;
  }

  // Restore the sidebar the way it was last left.
  var initialSidebar = root.getAttribute("data-mdv-sidebar");
  if ((initialSidebar === "files" && !hasFiles) || (initialSidebar === "links" && !served)) initialSidebar = "contents";
  if (initialSidebar === "contents" || initialSidebar === "files" || initialSidebar === "links") {
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
        type: "button", "class": "mdv-wrap-btn mdv-ui", title: "Wrap long lines", "aria-label": "Wrap long lines",
        "aria-pressed": "false",
      }, "\u21a9"), el("button", {
        type: "button", "class": "mdv-copy mdv-ui", title: "Copy", "aria-label": "Copy code",
      }, icon("copy")));
    });
  }
  document.addEventListener("click", function (e) {
    var wrapBtn = e.target.closest && e.target.closest(".mdv-wrap-btn");
    if (wrapBtn) {
      var pre = wrapBtn.parentElement.querySelector("pre");
      var on = pre.classList.toggle("mdv-wrapped");
      wrapBtn.setAttribute("aria-pressed", String(on));
      return;
    }
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

  // ---- Sortable tables -----------------------------------------------------
  // Click a header to sort by that column: ascending, descending, then back
  // to the file's order. Sorting is the reader's view, not a change to the
  // document: live.js calls mdvSort.save() before an update (restoring the
  // file's order, which its source positions follow) and restore() after.
  function sortable(table) {
    return table.tHead && table.tBodies.length === 1 && table.tBodies[0].rows.length > 1 &&
      !table.closest(".mdv-ui");
  }
  function markSortable(container) {
    container.querySelectorAll("table").forEach(function (t) {
      if (sortable(t)) t.classList.add("mdv-sortable");
    });
  }
  markSortable(body);
  (window.mdvEnhancers = window.mdvEnhancers || []).push(markSortable);

  var NUMBER = /^[-+]?[$€£¥]?\s*[-+]?(\d[\d,]*\.?\d*|\.\d+)(e[-+]?\d+)?\s*%?$/i;
  function sortKey(cell) {
    var text = cell ? cell.textContent.trim() : "";
    if (NUMBER.test(text)) {
      var n = parseFloat(text.replace(/[^\d.eE+-]/g, ""));
      if (isFinite(n)) return { n: n, t: text };
    }
    return { n: null, t: text };
  }
  var collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
  function sortTable(table, col, dir) {
    var tbody = table.tBodies[0];
    var rows = Array.prototype.slice.call(tbody.rows);
    rows.forEach(function (r, i) {
      if (!r.hasAttribute("data-mdv-order")) r.setAttribute("data-mdv-order", String(i));
    });
    var order = function (r) { return +r.getAttribute("data-mdv-order"); };
    if (dir) {
      var keys = new Map(rows.map(function (r) { return [r, sortKey(r.cells[col])]; }));
      var sign = dir === "descending" ? -1 : 1;
      rows.sort(function (a, b) {
        var x = keys.get(a), y = keys.get(b);
        // Empty cells last, whichever way.
        if (!x.t !== !y.t) return x.t ? -1 : 1;
        var c = x.n !== null && y.n !== null ? x.n - y.n
          : x.n !== null ? -1 : y.n !== null ? 1 : collator.compare(x.t, y.t);
        return c * sign || order(a) - order(b);
      });
    } else {
      rows.sort(function (a, b) { return order(a) - order(b); });
    }
    rows.forEach(function (r) {
      if (!dir) r.removeAttribute("data-mdv-order");
      tbody.appendChild(r);
    });
    Array.prototype.forEach.call(table.tHead.rows[0].cells, function (th, i) {
      if (dir && i === col) th.setAttribute("aria-sort", dir);
      else th.removeAttribute("aria-sort");
    });
  }
  body.addEventListener("click", function (e) {
    var th = e.target.closest && e.target.closest("thead th");
    if (!th || e.target.closest("a")) return;
    var table = th.closest("table");
    if (!table || !body.contains(table) || !sortable(table)) return;
    var now = th.getAttribute("aria-sort");
    var next = now === "ascending" ? "descending" : now === "descending" ? null : "ascending";
    sortTable(table, th.cellIndex, next);
  });

  function tableSignature(table) {
    return table.tHead ? table.tHead.textContent.replace(/\s+/g, " ").trim() : "";
  }
  window.mdvSort = {
    // Put every sorted table back in the file's order; returns what to redo.
    save: function (container) {
      var saved = [];
      container.querySelectorAll("table").forEach(function (t, i) {
        var th = t.tHead && t.tHead.querySelector("th[aria-sort]");
        if (!th) return;
        saved.push([i, tableSignature(t), th.cellIndex, th.getAttribute("aria-sort")]);
        sortTable(t, th.cellIndex, null);
      });
      return saved;
    },
    restore: function (container, saved) {
      var tables = container.querySelectorAll("table");
      saved.forEach(function (s) {
        var t = tables[s[0]];
        if (t && sortable(t) && tableSignature(t) === s[1]) sortTable(t, s[2], s[3]);
      });
    },
  };

  // ---- Task lists ----------------------------------------------------------
  // On served Markdown files, ticking a box changes its [ ] in the file;
  // the page then updates as after any other edit. Boxes without a source
  // position (embedded notes, exports) stay read-only.
  function enableTasks(container) {
    if (!canEdit) return;
    container.querySelectorAll("input.task-list-item-checkbox").forEach(function (box) {
      var li = box.closest("li");
      if (!li || !li.hasAttribute("data-sourcepos") || box.closest(".mdv-embed-note")) return;
      box.removeAttribute("disabled");
      box.title = "Tick to update the file";
    });
  }
  enableTasks(body);
  (window.mdvEnhancers = window.mdvEnhancers || []).push(enableTasks);
  body.addEventListener("change", function (e) {
    var box = e.target;
    if (!box.classList || !box.classList.contains("task-list-item-checkbox")) return;
    var li = box.closest("li");
    var pos = li && /^(\d+):(\d+)/.exec(li.getAttribute("data-sourcepos") || "");
    if (!pos) return;
    fetch("/__mdv/task", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-MDV": "1" },
      body: JSON.stringify({ doc: currentDoc, line: +pos[1], col: +pos[2], done: box.checked }),
    }).then(function (r) {
      if (!r.ok) return r.text().then(function (t) { throw new Error(t || "error " + r.status); });
    }).catch(function (err) {
      box.checked = !box.checked;
      showToast("Couldn't update the file: " + err.message);
    });
  });

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
    if (zoomOpen()) {
      if (key === "Escape") closeZoom(); else return;
      e.preventDefault();
      return;
    }
    if (presenting()) {
      if (presentKey(key)) e.preventDefault();
      return;
    }
    switch (key) {
      case "t": cycleTheme(); break;
      case "o": toggleSidebar("contents"); break;
      // "Find a file" always opens the list and focuses the filter.
      case "f": if (!hasFiles) return; openSidebar("files", true); break;
      case "/": if (!served) return; openSidebar("files", true); break;
      case "e": if (!canEdit) return; editAt(blockAtTop()); break;
      case "s": startPresenting(); break;
      case "+": case "=": stepScale(1); break;
      case "-": case "_": stepScale(-1); break;
      case "0": setScale(1); break;
      case "c": copyPage(); break;
      case "b": if (!served) return; openSidebar("links"); break;
      case "l": if (!served) return; checkLinks(); break;
      case "D": if (!served) return; toggleGit(); break;
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
        else if (!linksOverlay.hidden) linksOverlay.hidden = true;
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
    if (gitOn) refreshGitSoon();
    if (presenting()) { buildSlides(); showSlide(slideIndex); return; }
    var changed = e.detail && e.detail.changed;
    if (!changed) return;
    var r = changed.getBoundingClientRect();
    if (r.bottom < 0 || r.top > window.innerHeight) changed.scrollIntoView({ block: "center" });
    changed.classList.add("mdv-flash");
    setTimeout(function () { changed.classList.remove("mdv-flash"); }, 1700);
  });

  // ---- Source positions -------------------------------------------------
  function lineOf(node) {
    var pos = node && node.getAttribute && node.getAttribute("data-sourcepos");
    return pos ? parseInt(pos, 10) : 0;
  }
  function range(node) {
    var m = /^(\d+):\d+-(\d+):/.exec(node.getAttribute("data-sourcepos") || "");
    return m ? [+m[1], +m[2]] : null;
  }
  // The block at the top of the window: the deepest element that spans the
  // reading line, else the first one below it.
  function blockAtTop() {
    var y = 80, found = null;
    var nodes = body.querySelectorAll("[data-sourcepos]");
    for (var i = 0; i < nodes.length; i++) {
      var r = nodes[i].getBoundingClientRect();
      if (r.height === 0) continue;
      if (r.top <= y && r.bottom > y) found = nodes[i];
      else if (r.top > y) return found || nodes[i];
    }
    return found;
  }
  // The deepest element whose source range contains `line`.
  function blockForLine(line) {
    var best = null, next = null;
    var nodes = body.querySelectorAll("[data-sourcepos]");
    for (var i = 0; i < nodes.length; i++) {
      var rg = range(nodes[i]);
      if (!rg) continue;
      if (rg[0] <= line && line <= rg[1]) best = nodes[i];
      else if (rg[0] > line && !next) next = nodes[i];
    }
    return best || next;
  }

  // ---- Click to jump to source -------------------------------------------
  function editAt(node) {
    var line = lineOf(node) || 1;
    fetch("/__mdv/open", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-MDV": "1" },
      body: JSON.stringify({ doc: currentDoc, line: line }),
    }).then(function (r) {
      if (r.ok) showToast("Opening the source at line " + line);
      else r.text().then(function (t) { showToast(t || "Couldn't open the editor"); });
    }).catch(function () { showToast("Couldn't open the editor"); });
  }
  editBtn.addEventListener("click", function () { editAt(blockAtTop()); });
  body.addEventListener("dblclick", function (e) {
    if (!canEdit) return;
    var t = e.target;
    if (t.closest("a, button, input, .mdv-ui, img, div.mermaid")) return;
    var node = t.closest("[data-sourcepos]");
    if (!node) return;
    var sel = window.getSelection && window.getSelection();
    if (sel) sel.removeAllRanges();
    editAt(node);
  });

  // ---- Reading position ---------------------------------------------------
  var userScrolled = false;
  ["wheel", "touchmove", "keydown", "mousedown"].forEach(function (ev) {
    window.addEventListener(ev, function () { userScrolled = true; }, { passive: true, once: true });
  });
  var positionKey = "mdv-pos:" + location.pathname;
  var lastSavedLine = 0, positionTimer;
  function savePosition(keepalive) {
    if (served) {
      var node = blockAtTop();
      var line = lineOf(node);
      if (!line || line === lastSavedLine) return;
      lastSavedLine = line;
      fetch("/__mdv/position", {
        method: "POST",
        keepalive: !!keepalive,
        headers: { "Content-Type": "application/json", "X-MDV": "1" },
        body: JSON.stringify({ doc: currentDoc, line: line }),
      }).catch(function () {});
    } else {
      try { localStorage.setItem(positionKey, String(Math.round(window.scrollY))); } catch (e) {}
    }
  }
  window.addEventListener("scroll", function () {
    clearTimeout(positionTimer);
    positionTimer = setTimeout(savePosition, 1000);
  }, { passive: true });
  window.addEventListener("pagehide", function () { savePosition(true); });

  function restorePosition() {
    var hash = /^#mdv-line-(\d+)$/.exec(location.hash);
    if (hash) {
      revealLine(+hash[1]);
      return;
    }
    if (location.hash) return;
    if (served) {
      var line = +root.getAttribute("data-mdv-resume") || 0;
      if (line <= 1) return;
      var node = blockForLine(line);
      if (node) {
        lastSavedLine = line;
        window.scrollTo(0, window.scrollY + node.getBoundingClientRect().top - 60);
      }
    } else {
      var y = 0;
      try { y = +localStorage.getItem(positionKey) || 0; } catch (e) {}
      if (y > 0) window.scrollTo(0, y);
    }
  }
  // Diagrams and math change the page's height as they render, so restore
  // again shortly after, unless the reader has started scrolling.
  restorePosition();
  [400, 1200].forEach(function (ms) {
    setTimeout(function () { if (!userScrolled) restorePosition(); }, ms);
  });
  window.addEventListener("hashchange", restorePosition);

  // ---- Image and diagram zoom ---------------------------------------------
  var zoom = null;
  function zoomOpen() { return !!zoom; }
  function openZoom(content) {
    var layer = el("div", { "class": "mdv-ui mdv-zoom", role: "dialog", "aria-label": "Zoomed image" });
    var stage = el("div", { "class": "mdv-zoom-stage" });
    stage.append(content);
    layer.append(stage);
    document.body.append(layer);
    zoom = { layer: layer, stage: stage, scale: 1, x: 0, y: 0 };
    applyZoom();
    var drag = null, moved = false;
    layer.addEventListener("wheel", function (e) {
      e.preventDefault();
      zoom.scale = Math.min(8, Math.max(0.2, zoom.scale * (e.deltaY < 0 ? 1.15 : 1 / 1.15)));
      applyZoom();
    }, { passive: false });
    layer.addEventListener("mousedown", function (e) {
      drag = { x: e.clientX - zoom.x, y: e.clientY - zoom.y };
      moved = false;
      layer.classList.add("mdv-dragging");
    });
    layer.addEventListener("mousemove", function (e) {
      if (!drag) return;
      moved = true;
      zoom.x = e.clientX - drag.x;
      zoom.y = e.clientY - drag.y;
      applyZoom();
    });
    layer.addEventListener("mouseup", function () {
      drag = null;
      layer.classList.remove("mdv-dragging");
      if (!moved) closeZoom();
    });
    layer.addEventListener("dblclick", function (e) {
      e.stopPropagation();
      zoom.scale = 1; zoom.x = 0; zoom.y = 0;
      applyZoom();
    });
  }
  function applyZoom() {
    zoom.stage.style.transform = "translate(" + zoom.x + "px," + zoom.y + "px) translate(-50%,-50%) scale(" + zoom.scale + ")";
  }
  function closeZoom() {
    if (!zoom) return;
    zoom.layer.remove();
    zoom = null;
  }
  body.addEventListener("click", function (e) {
    var img = e.target.closest("img");
    if (img && !img.closest("a")) {
      var copy = el("img", { src: img.currentSrc || img.src, alt: img.alt || "" });
      openZoom(copy);
      return;
    }
    var diagram = e.target.closest("div.mermaid");
    if (diagram && diagram.querySelector("svg")) {
      var svg = diagram.querySelector("svg").cloneNode(true);
      var vb = svg.viewBox && svg.viewBox.baseVal;
      svg.removeAttribute("style");
      svg.style.width = Math.min(window.innerWidth * 0.92, (vb && vb.width ? vb.width : 800) * 1.6) + "px";
      svg.style.height = "auto";
      openZoom(svg);
    }
  });

  // ---- Presentation mode ---------------------------------------------------
  // Slides are split at horizontal rules (---) if there are any, else at
  // each h1 and h2.
  var deck = null, slides = [], slideIndex = 0;
  function presenting() { return !!deck; }
  function buildSlides() {
    // Front matter is metadata, not a slide.
    var kids = Array.prototype.filter.call(body.children, function (k) {
      return !k.classList.contains("mdv-front-matter");
    });
    var byRule = kids.some(function (k) { return k.tagName === "HR"; });
    slides = [[]];
    kids.forEach(function (k) {
      if (byRule && k.tagName === "HR") { slides.push([]); return; }
      if (!byRule && /^H[12]$/.test(k.tagName) && slides[slides.length - 1].length) slides.push([]);
      slides[slides.length - 1].push(k);
    });
    slides = slides.filter(function (s) { return s.length; });
    if (!slides.length) slides = [[]];
  }
  function slideFor(node) {
    if (!node) return 0;
    for (var i = 0; i < slides.length; i++) {
      for (var j = 0; j < slides[i].length; j++) {
        if (slides[i][j] === node || slides[i][j].contains(node)) return i;
      }
    }
    return 0;
  }
  function startPresenting() {
    buildSlides();
    deck = el("div", { "class": "mdv-ui mdv-present", role: "dialog", "aria-label": "Presentation" });
    var page = el("div", { "class": "markdown-body mdv-slide" });
    var bar = el("div", { "class": "mdv-present-bar" });
    deck.append(page, bar);
    document.body.append(deck);
    deck.addEventListener("click", function (e) {
      if (e.target.closest("a, button, input, details, .mdv-copy")) return;
      showSlide(slideIndex + (e.clientX < window.innerWidth / 3 ? -1 : 1));
    });
    if (deck.requestFullscreen) deck.requestFullscreen().catch(function () {});
    showSlide(slideFor(blockAtTop()));
  }
  function showSlide(i) {
    slideIndex = Math.max(0, Math.min(slides.length - 1, i));
    var page = deck.querySelector(".mdv-slide");
    var inner = el("div", { "class": "mdv-slide-inner" });
    slides[slideIndex].forEach(function (n) { inner.append(n.cloneNode(true)); });
    page.replaceChildren(inner);
    page.scrollTop = 0;
    deck.querySelector(".mdv-present-bar").textContent = (slideIndex + 1) + " / " + slides.length;
  }
  function stopPresenting() {
    if (document.fullscreenElement) document.exitFullscreen().catch(function () {});
    deck.remove();
    deck = null;
  }
  document.addEventListener("fullscreenchange", function () {
    if (deck && !document.fullscreenElement && deck.dataset.wasFull) stopPresenting();
    if (deck && document.fullscreenElement) deck.dataset.wasFull = "1";
  });
  function presentKey(key) {
    switch (key) {
      case "ArrowRight": case "ArrowDown": case "PageDown": case " ": case "j": case "n":
        showSlide(slideIndex + 1); return true;
      case "ArrowLeft": case "ArrowUp": case "PageUp": case "Backspace": case "k": case "p":
        showSlide(slideIndex - 1); return true;
      case "Home": showSlide(0); return true;
      case "End": showSlide(slides.length - 1); return true;
      case "Escape": case "s": case "q": stopPresenting(); return true;
      case "t": cycleTheme(); return true;
      default: return false;
    }
  }
  presentBtn.addEventListener("click", startPresenting);

  // ---- Folding Obsidian callouts -------------------------------------------
  document.addEventListener("click", function (e) {
    var title = e.target.closest && e.target.closest(".callout-title");
    if (!title) return;
    var callout = title.closest(".callout");
    var fold = callout && callout.getAttribute("data-callout-fold");
    if (!fold || fold === "none") return;
    setFold(callout, fold === "collapsed" ? "expanded" : "collapsed");
  });
  // Remember how the callout was rendered, so live updates can tell the
  // reader's folding apart from edits (see live.js).
  function setFold(callout, value) {
    if (!callout.hasAttribute("data-mdv-fold-orig")) {
      callout.setAttribute("data-mdv-fold-orig", callout.getAttribute("data-callout-fold"));
    }
    callout.setAttribute("data-callout-fold", value);
  }

  // ---- Going to a line (search results, --goto, broken-link list) ---------
  function revealLine(line) {
    var target = blockForLine(line);
    if (!target) return;
    // Unfold any folded callout around it, or it would stay hidden.
    for (var c = target.closest(".callout"); c; c = c.parentElement && c.parentElement.closest(".callout")) {
      if (c.getAttribute("data-callout-fold") === "collapsed") setFold(c, "expanded");
    }
    target.scrollIntoView({ block: "center" });
    target.classList.add("mdv-flash");
    setTimeout(function () { target.classList.remove("mdv-flash"); }, 1700);
  }
  document.addEventListener("mdv:goto", function (e) { revealLine(e.detail.line); });

  // ---- Text size ----------------------------------------------------------
  // Served pages get a new port each run, so the browser's own zoom isn't
  // remembered; this is.
  var SCALES = [0.6, 0.7, 0.8, 0.9, 1, 1.1, 1.25, 1.4, 1.6, 1.8, 2, 2.4, 3];
  function scale() {
    return parseFloat(root.style.getPropertyValue("--mdv-scale")) || 1;
  }
  function setScale(s) {
    if (s === 1) root.style.removeProperty("--mdv-scale");
    else root.style.setProperty("--mdv-scale", String(s));
    savePrefs({ font_scale: s });
    showToast("Text size " + Math.round(s * 100) + "%");
  }
  function stepScale(dir) {
    var cur = scale();
    var next = dir > 0
      ? SCALES.filter(function (v) { return v > cur + 0.001; })[0]
      : SCALES.filter(function (v) { return v < cur - 0.001; }).pop();
    if (next) setScale(next);
  }
  if (!served) {
    var localScale = loadLocal().font_scale;
    if (localScale && localScale !== 1) root.style.setProperty("--mdv-scale", String(localScale));
  }

  // ---- Copy the page --------------------------------------------------------
  // Rich text (for Docs, Slack, email) plus the Markdown as plain text.
  var PASTE_STYLES = {
    pre: "background:#f6f8fa;padding:12px;border-radius:6px;font-family:ui-monospace,Menlo,monospace;font-size:13px;white-space:pre-wrap",
    code: "background:#f6f8fa;padding:1px 4px;border-radius:4px;font-family:ui-monospace,Menlo,monospace",
    table: "border-collapse:collapse",
    th: "border:1px solid #d0d7de;padding:6px 12px;background:#f6f8fa",
    td: "border:1px solid #d0d7de;padding:6px 12px",
    blockquote: "border-left:4px solid #d0d7de;margin-left:0;padding-left:12px;color:#59636e",
  };
  function pageHtml() {
    var clone = body.cloneNode(true);
    var pres = body.querySelectorAll("pre");
    clone.querySelectorAll("pre").forEach(function (pre, i) {
      // Plain code text, with line breaks (rendered lines are blocks).
      pre.textContent = pres[i] ? pres[i].innerText : pre.textContent;
    });
    clone.querySelectorAll(".mdv-ui, .anchor, .mdv-copy, .mdv-wrap-btn").forEach(function (n) { n.remove(); });
    // Math as its TeX source, which pastes everywhere.
    clone.querySelectorAll(".math").forEach(function (m) {
      var tex = m.querySelector("annotation");
      var code = document.createElement("code");
      code.textContent = tex ? (m.classList.contains("math-display") ? "$$" + tex.textContent + "$$" : "$" + tex.textContent + "$") : m.textContent;
      m.replaceWith(code);
    });
    clone.querySelectorAll("[data-sourcepos]").forEach(function (n) { n.removeAttribute("data-sourcepos"); });
    clone.querySelectorAll("img").forEach(function (img) { img.setAttribute("src", img.src); });
    clone.querySelectorAll("a[href]").forEach(function (a) { a.setAttribute("href", a.href); });
    Object.keys(PASTE_STYLES).forEach(function (tag) {
      clone.querySelectorAll(tag).forEach(function (n) {
        if (tag === "code" && n.parentElement && n.parentElement.tagName === "PRE") return;
        n.setAttribute("style", PASTE_STYLES[tag]);
      });
    });
    return clone.innerHTML;
  }
  function copyPage() {
    var html = pageHtml();
    var text = served
      ? fetch("/__mdv/source?doc=" + encodeURIComponent(currentDoc))
          .then(function (r) { return r.ok ? r.text() : body.innerText; })
          .catch(function () { return body.innerText; })
      : Promise.resolve(body.innerText);
    text.then(function (plain) {
      if (window.ClipboardItem && navigator.clipboard && navigator.clipboard.write) {
        return navigator.clipboard.write([new ClipboardItem({
          "text/html": new Blob([html], { type: "text/html" }),
          "text/plain": new Blob([plain], { type: "text/plain" }),
        })]);
      }
      return navigator.clipboard.writeText(plain);
    }).then(function () {
      showToast("Copied the page");
    }, function () {
      showToast("Couldn't copy (the browser refused)");
    });
  }
  copyPageBtn.addEventListener("click", copyPage);

  // ---- Checking links ---------------------------------------------------------
  var linksOverlay = el("div", { "class": "mdv-ui mdv-overlay", hidden: "" });
  var linksBox = el("div", { "class": "mdv-help mdv-broken", role: "dialog", "aria-label": "Broken links" });
  linksOverlay.append(linksBox);
  document.body.append(linksOverlay);
  linksOverlay.addEventListener("click", function (e) { if (e.target === linksOverlay) linksOverlay.hidden = true; });
  function checkLinks() {
    showToast("Checking links…");
    fetch("/__mdv/check?doc=" + encodeURIComponent(currentDoc)).then(function (r) { return r.json(); }).then(function (data) {
      var broken = data.broken || [];
      var web = data.external ? " (" + data.external + " web links not checked)" : "";
      if (!broken.length) { showToast("No broken links" + web); return; }
      linksBox.textContent = "";
      var h = el("h2");
      h.textContent = broken.length + (broken.length === 1 ? " broken link" : " broken links");
      linksBox.append(h);
      broken.forEach(function (b) {
        var item = el("button", { type: "button", "class": "mdv-broken-item" });
        var where = el("span", { "class": "mdv-hit-file" });
        where.textContent = "Line " + b.line + ": " + b.reason;
        var what = el("span", { "class": "mdv-hit-text" });
        what.textContent = b.target;
        item.append(where, what);
        item.addEventListener("click", function () { linksOverlay.hidden = true; revealLine(b.line); });
        linksBox.append(item);
      });
      if (web) {
        var note = el("p", { "class": "mdv-hit-file" });
        note.textContent = web.trim() + "; markdown-viewer --check-links --online checks them.";
        linksBox.append(note);
      }
      linksOverlay.hidden = false;
    }).catch(function () { showToast("Couldn't check links"); });
  }

  // ---- Changes since the last git commit -------------------------------------
  var gitOn = root.hasAttribute("data-mdv-git");
  var INLINE = /^(A|ABBR|B|CODE|DEL|EM|I|IMG|INPUT|KBD|MARK|S|SPAN|STRONG|SUB|SUP|U)$/;
  function clearGit() {
    body.querySelectorAll(".mdv-git-added, .mdv-git-modified, .mdv-git-deleted").forEach(function (n) {
      n.classList.remove("mdv-git-added", "mdv-git-modified", "mdv-git-deleted");
      if (!n.getAttribute("class")) n.removeAttribute("class");
    });
  }
  function applyGit(announce) {
    return fetch("/__mdv/gitdiff?doc=" + encodeURIComponent(currentDoc)).then(function (r) { return r.json(); }).then(function (d) {
      clearGit();
      if (!gitOn) return;
      if (d.status === "not-a-repo") { if (announce) showToast("Not in a git repository"); return; }
      if (announce) {
        showToast(d.status === "untracked" ? "Not committed yet: everything is new"
          : d.changes.length ? "Showing changes since the last commit" : "No changes since the last commit");
      }
      var blocks = Array.prototype.filter.call(body.querySelectorAll("[data-sourcepos]"), function (n) {
        return !INLINE.test(n.tagName) && range(n);
      });
      d.changes.forEach(function (ch) {
        if (ch.kind === "deleted") {
          var next = Array.prototype.filter.call(body.children, function (n) {
            var r = range(n);
            return r && r[0] > ch.start;
          })[0];
          if (next) next.classList.add("mdv-git-deleted");
          return;
        }
        var hits = blocks.filter(function (n) {
          var r = range(n);
          return r[0] <= ch.end && r[1] >= ch.start;
        });
        // Mark the innermost blocks only (a list item, not the whole list).
        hits.filter(function (n) {
          return !hits.some(function (m) { return m !== n && n.contains(m); });
        }).forEach(function (n) {
          // Code blocks clip anything outside their edges (they scroll
          // sideways), so mark the wrapper around them.
          var target = n.tagName === "PRE"
            ? n.closest(".mdv-code-figure") || n.closest(".mdv-code") || n
            : n;
          target.classList.add(ch.kind === "added" ? "mdv-git-added" : "mdv-git-modified");
        });
      });
    }).catch(function () {});
  }
  var gitTimer;
  function refreshGitSoon() {
    clearTimeout(gitTimer);
    gitTimer = setTimeout(function () { applyGit(false); }, 300);
  }
  function toggleGit() {
    gitOn = !gitOn;
    savePrefs({ git: gitOn });
    if (gitOn) applyGit(true);
    else { clearGit(); showToast("Not showing git changes"); }
  }
  if (gitOn && served) setTimeout(function () { applyGit(false); }, 300);

  // ---- Backlinks ----------------------------------------------------------------
  function loadBacklinks() {
    linksPanel.textContent = "";
    linksPanel.append(el("div", { "class": "mdv-empty" }, "Loading…"));
    fetch("/__mdv/backlinks?doc=" + encodeURIComponent(currentDoc)).then(function (r) { return r.json(); }).then(function (data) {
      linksPanel.textContent = "";
      var hits = data.hits || [];
      var heading = el("div", { "class": "mdv-dir" });
      heading.textContent = hits.length ? "Linked from" : "No other files link here";
      linksPanel.append(heading);
      hits.forEach(function (h) {
        var a = el("a", {
          "class": "mdv-hit",
          href: "/" + h.file.split("/").map(encodeURIComponent).join("/") + "#mdv-line-" + h.line,
          title: h.file + ":" + h.line,
        });
        var where = el("span", { "class": "mdv-hit-file" });
        where.textContent = h.file + ":" + h.line;
        var what = el("span", { "class": "mdv-hit-text" });
        what.textContent = h.text;
        a.append(where, what);
        linksPanel.append(a);
      });
    }).catch(function () {
      linksPanel.textContent = "";
      linksPanel.append(el("div", { "class": "mdv-empty" }, "Couldn't load backlinks"));
    });
  }
  linksTab.addEventListener("click", function () { openSidebar("links"); });

  addCopyButtons(body);
  buildContents();
  updateStats();
})();
