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
  var LONG_PAGE = 150;
  // For styles that only make sense with this script (the STL drag hint).
  root.setAttribute("data-mdv-ui", "");
  var body = document.querySelector(".markdown-body");
  // Long pages: blocks off screen aren't laid out or drawn until they come
  // near (content-visibility in ui.css), so they open and resize quickly.
  if (body && body.childElementCount >= LONG_PAGE) root.setAttribute("data-mdv-long", "");

  // Icons: Primer Octicons (MIT), see assets/LICENSE-octicons.
  var ICONS = {
    "tag": ["M1 7.775V2.75C1 1.784 1.784 1 2.75 1h5.025c.464 0 .91.184 1.238.513l6.25 6.25a1.75 1.75 0 0 1 0 2.474l-5.026 5.026a1.75 1.75 0 0 1-2.474 0l-6.25-6.25A1.752 1.752 0 0 1 1 7.775Zm1.5 0c0 .066.026.13.073.177l6.25 6.25a.25.25 0 0 0 .354 0l5.025-5.025a.25.25 0 0 0 0-.354l-6.25-6.25a.25.25 0 0 0-.177-.073H2.75a.25.25 0 0 0-.25.25ZM6 5a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z"],
    "code": ["m11.28 3.22 4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.749.749 0 0 1-1.275-.326.749.749 0 0 1 .215-.734L13.94 8l-3.72-3.72a.749.749 0 0 1 .326-1.275.749.749 0 0 1 .734.215Zm-6.56 0a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042L2.06 8l3.72 3.72a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L.47 8.53a.75.75 0 0 1 0-1.06Z"],
    "gear": ["M8 0a8.2 8.2 0 0 1 .701.031C9.444.095 9.99.645 10.16 1.29l.288 1.107c.018.066.079.158.212.224.231.114.454.243.668.386.123.082.233.09.299.071l1.103-.303c.644-.176 1.392.021 1.82.63.27.385.506.792.704 1.218.315.675.111 1.422-.364 1.891l-.814.806c-.049.048-.098.147-.088.294.016.257.016.515 0 .772-.01.147.038.246.088.294l.814.806c.475.469.679 1.216.364 1.891a7.977 7.977 0 0 1-.704 1.217c-.428.61-1.176.807-1.82.63l-1.102-.302c-.067-.019-.177-.011-.3.071a5.909 5.909 0 0 1-.668.386c-.133.066-.194.158-.211.224l-.29 1.106c-.168.646-.715 1.196-1.458 1.26a8.006 8.006 0 0 1-1.402 0c-.743-.064-1.289-.614-1.458-1.26l-.289-1.106c-.018-.066-.079-.158-.212-.224a5.738 5.738 0 0 1-.668-.386c-.123-.082-.233-.09-.299-.071l-1.103.303c-.644.176-1.392-.021-1.82-.63a8.12 8.12 0 0 1-.704-1.218c-.315-.675-.111-1.422.363-1.891l.815-.806c.05-.048.098-.147.088-.294a6.214 6.214 0 0 1 0-.772c.01-.147-.038-.246-.088-.294l-.815-.806C.635 6.045.431 5.298.746 4.623a7.92 7.92 0 0 1 .704-1.217c.428-.61 1.176-.807 1.82-.63l1.102.302c.067.019.177.011.3-.071.214-.143.437-.272.668-.386.133-.066.194-.158.211-.224l.29-1.106C6.009.645 6.556.095 7.299.03 7.53.01 7.764 0 8 0Zm-.571 1.525c-.036.003-.108.036-.137.146l-.289 1.105c-.147.561-.549.967-.998 1.189-.173.086-.34.183-.5.29-.417.278-.97.423-1.529.27l-1.103-.303c-.109-.03-.175.016-.195.045-.22.312-.412.644-.573.99-.014.031-.021.11.059.19l.815.806c.411.406.562.957.53 1.456a4.709 4.709 0 0 0 0 .582c.032.499-.119 1.05-.53 1.456l-.815.806c-.081.08-.073.159-.059.19.162.346.353.677.573.989.02.03.085.076.195.046l1.102-.303c.56-.153 1.113-.008 1.53.27.161.107.328.204.501.29.447.222.85.629.997 1.189l.289 1.105c.029.109.101.143.137.146a6.6 6.6 0 0 0 1.142 0c.036-.003.108-.036.137-.146l.289-1.105c.147-.561.549-.967.998-1.189.173-.086.34-.183.5-.29.417-.278.97-.423 1.529-.27l1.103.303c.109.029.175-.016.195-.045.22-.313.411-.644.573-.99.014-.031.021-.11-.059-.19l-.815-.806c-.411-.406-.562-.957-.53-1.456a4.709 4.709 0 0 0 0-.582c-.032-.499.119-1.05.53-1.456l.815-.806c.081-.08.073-.159.059-.19a6.464 6.464 0 0 0-.573-.989c-.02-.03-.085-.076-.195-.046l-1.102.303c-.56.153-1.113.008-1.53-.27a4.44 4.44 0 0 0-.501-.29c-.447-.222-.85-.629-.997-1.189l-.289-1.105c-.029-.11-.101-.143-.137-.146a6.6 6.6 0 0 0-1.142 0ZM11 8a3 3 0 1 1-6 0 3 3 0 0 1 6 0ZM9.5 8a1.5 1.5 0 1 0-3.001.001A1.5 1.5 0 0 0 9.5 8Z"],
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
    "file-directory-open-fill": ["M.513 1.513A1.75 1.75 0 0 1 1.75 1h3.5c.55 0 1.07.26 1.4.7l.9 1.2a.25.25 0 0 0 .2.1H13a1 1 0 0 1 1 1v.5H2.75a.75.75 0 0 0 0 1.5h11.978a1 1 0 0 1 .994 1.117L15 13.25A1.75 1.75 0 0 1 13.25 15H1.75A1.75 1.75 0 0 1 0 13.25V2.75c0-.464.184-.91.513-1.237Z"],
    "book": ["M0 1.75A.75.75 0 0 1 .75 1h4.253c1.227 0 2.317.59 3 1.501A3.743 3.743 0 0 1 11.006 1h4.245a.75.75 0 0 1 .75.75v10.5a.75.75 0 0 1-.75.75h-4.507a2.25 2.25 0 0 0-1.591.659l-.622.621a.75.75 0 0 1-1.06 0l-.622-.621A2.25 2.25 0 0 0 5.258 13H.75a.75.75 0 0 1-.75-.75Zm7.251 10.324.004-5.073-.002-2.253A2.25 2.25 0 0 0 5.003 2.5H1.5v9h3.757a3.75 3.75 0 0 1 1.994.574ZM8.755 4.75l-.004 7.322a3.752 3.752 0 0 1 1.992-.572H14.5v-9h-3.495a2.25 2.25 0 0 0-2.25 2.25Z"],
    "table": ["M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v12.5A1.75 1.75 0 0 1 14.25 16H1.75A1.75 1.75 0 0 1 0 14.25ZM6.5 6.5v8h7.75a.25.25 0 0 0 .25-.25V6.5Zm8-1.5V1.75a.25.25 0 0 0-.25-.25H6.5V5Zm-13 1.5v7.75c0 .138.112.25.25.25H5v-8ZM5 5V1.5H1.75a.25.25 0 0 0-.25.25V5Z"],
    "arrow-left": ["M7.78 12.53a.75.75 0 0 1-1.06 0L2.47 8.28a.75.75 0 0 1 0-1.06l4.25-4.25a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042L4.81 7h7.44a.75.75 0 0 1 0 1.5H4.81l2.97 2.97a.75.75 0 0 1 0 1.06Z"],
    "arrow-right": ["M8.22 2.97a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042l2.97-2.97H3.75a.75.75 0 0 1 0-1.5h7.44L8.22 4.03a.75.75 0 0 1 0-1.06Z"],
    "sync": ["M1.705 8.005a.75.75 0 0 1 .834.656 5.5 5.5 0 0 0 9.592 2.97l-1.204-1.204a.25.25 0 0 1 .177-.427h3.646a.25.25 0 0 1 .25.25v3.646a.25.25 0 0 1-.427.177l-1.38-1.38A7.002 7.002 0 0 1 1.05 8.84a.75.75 0 0 1 .656-.834ZM8 2.5a5.487 5.487 0 0 0-4.131 1.869l1.204 1.204A.25.25 0 0 1 4.896 6H1.25A.25.25 0 0 1 1 5.75V2.104a.25.25 0 0 1 .427-.177l1.38 1.38A7.002 7.002 0 0 1 14.95 7.16a.75.75 0 0 1-1.49.178A5.5 5.5 0 0 0 8 2.5Z"],
    "history": ["m.427 1.927 1.215 1.215a8.002 8.002 0 1 1-1.6 5.685.75.75 0 1 1 1.493-.154 6.5 6.5 0 1 0 1.18-4.458l1.358 1.358A.25.25 0 0 1 3.896 6H.25A.25.25 0 0 1 0 5.75V2.104a.25.25 0 0 1 .427-.177ZM7.75 4a.75.75 0 0 1 .75.75v2.992l2.028.812a.75.75 0 0 1-.557 1.392l-2.5-1A.751.751 0 0 1 7 8.25v-3.5A.75.75 0 0 1 7.75 4Z"],
    "cross-reference": ["M2.75 3.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h4.5a.25.25 0 0 0 .25-.25v-2.5a.75.75 0 0 1 1.5 0v2.5A1.75 1.75 0 0 1 13.25 13H9.06l-2.573 2.573A1.458 1.458 0 0 1 4 14.543V13H2.75A1.75 1.75 0 0 1 1 11.25v-7.5C1 2.784 1.784 2 2.75 2h5.5a.75.75 0 0 1 0 1.5ZM16 1.25v4.146a.25.25 0 0 1-.427.177L14.03 4.03l-3.75 3.75a.749.749 0 0 1-1.275-.326.749.749 0 0 1 .215-.734l3.75-3.75-1.543-1.543A.25.25 0 0 1 11.604 1h4.146a.25.25 0 0 1 .25.25Z"],
    "file": ["M2 1.75C2 .784 2.784 0 3.75 0h6.586c.464 0 .909.184 1.237.513l2.914 2.914c.329.328.513.773.513 1.237v9.586A1.75 1.75 0 0 1 13.25 16h-9.5A1.75 1.75 0 0 1 2 14.25Zm1.75-.25a.25.25 0 0 0-.25.25v12.5c0 .138.112.25.25.25h9.5a.25.25 0 0 0 .25-.25V6h-2.75A1.75 1.75 0 0 1 9 4.25V1.5Zm6.75.062V4.25c0 .138.112.25.25.25h2.688l-.011-.013-2.914-2.914-.013-.011Z"],
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
    if (local.word_count === false) root.setAttribute("data-mdv-stats", "off");
  }

  // ---- Building the UI ---------------------------------------------------
  var toolbar = el("div", { "class": "mdv-ui mdv-toolbar", role: "toolbar", "aria-label": "Viewer" });
  var stats = el("span", { "class": "mdv-stats", title: "Words and reading time (click or w to hide)" });
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
  // Back, forward and reload: app windows have no browser controls.
  var backBtn = el("button", { type: "button", "class": "mdv-nav", title: "Back ([ or Alt-\u2190)", "aria-label": "Back" }, icon("arrow-left"));
  var forwardBtn = el("button", { type: "button", "class": "mdv-nav", title: "Forward (] or Alt-\u2192)", "aria-label": "Forward" }, icon("arrow-right"));
  var reloadBtn = el("button", { type: "button", "class": "mdv-nav mdv-nav-last", title: "Reload (r)", "aria-label": "Reload" }, icon("sync"));
  backBtn.addEventListener("click", function () { history.back(); });
  forwardBtn.addEventListener("click", function () { history.forward(); });
  reloadBtn.addEventListener("click", function () { location.reload(); });
  // Grey out what can't be done, where the browser can tell (the
  // Navigation API); elsewhere both stay enabled and do nothing at the ends.
  function updateNav() {
    var nav = window.navigation;
    if (!nav || typeof nav.canGoBack !== "boolean") return;
    backBtn.disabled = !nav.canGoBack;
    forwardBtn.disabled = !nav.canGoForward;
  }
  updateNav();
  if (window.navigation && window.navigation.addEventListener) {
    window.navigation.addEventListener("currententrychange", updateNav);
  }
  window.addEventListener("pageshow", updateNav);
  toolbar.append(backBtn, forwardBtn, reloadBtn, stats, contentsBtn);
  if (hasFiles) toolbar.append(filesBtn);
  if (canEdit) toolbar.append(editBtn);
  // The source view: pages whose Markdown the viewer has (files, and
  // standard input, the clipboard and web addresses).
  var canSource = served && script.hasAttribute("data-source");
  var sourceBtn = el("button", { type: "button", title: "View the Markdown source (v)", "aria-label": "View the Markdown source", "aria-pressed": "false" }, icon("code"));
  if (canSource) toolbar.append(sourceBtn);
  var copyPageBtn = el("button", { type: "button", title: "Copy the page (c)", "aria-label": "Copy the page" }, icon("copy"));
  var canSettings = served && script.hasAttribute("data-settings");
  var settingsBtn = el("button", { type: "button", title: "Settings (,)", "aria-label": "Settings" }, icon("gear"));
  toolbar.append(copyPageBtn, presentBtn, themeBtn);
  if (canSettings) toolbar.append(settingsBtn);
  toolbar.append(helpBtn);

  var sidebar = el("nav", { "class": "mdv-ui mdv-sidebar", "aria-label": "Document" });
  var tabs = el("div", { "class": "mdv-tabs", role: "tablist" });
  // Tabs are an icon and a name; only the selected tab shows its name, so
  // four of them fit the sidebar. The name is the tooltip and label.
  function tab(name, iconName, selected) {
    var b = el("button", { type: "button", role: "tab", "aria-selected": String(selected), title: name, "aria-label": name },
      icon(iconName));
    var label = el("span", { "class": "mdv-tab-label" });
    label.textContent = name;
    b.append(label);
    return b;
  }
  var contentsTab = tab("Contents", "list-unordered", true);
  var filesTab = tab("Files", "file-directory", false);
  var closeBtn = el("button", { type: "button", "class": "mdv-close", title: "Close (Esc)", "aria-label": "Close sidebar" }, icon("x"));
  tabs.append(contentsTab);
  var linksTab = tab("Backlinks", "cross-reference", false);
  if (hasFiles) tabs.append(filesTab);
  if (served) tabs.append(linksTab);
  var tagsTab = tab("Tags", "tag", false);
  if (served) tabs.append(tagsTab);
  // Recent files, when the viewer remembers them.
  var hasRecent = served && script.hasAttribute("data-recent");
  var recentTab = tab("Recent", "history", false);
  if (hasRecent) tabs.append(recentTab);
  tabs.append(closeBtn);
  var contentsPanel = el("div", { "class": "mdv-panel", role: "tabpanel" });
  var filesPanel = el("div", { "class": "mdv-panel", role: "tabpanel", hidden: "" });
  var filterLabel = served ? "Find files or text" : "Find files";
  var filter = el("input", { "class": "mdv-filter", type: "search", placeholder: filterLabel + "…", "aria-label": filterLabel });
  var fileList = el("div");
  var hitList = el("div");
  filesPanel.append(filter, fileList, hitList);
  var linksPanel = el("div", { "class": "mdv-panel", role: "tabpanel", hidden: "" });
  var recentPanel = el("div", { "class": "mdv-panel", role: "tabpanel", hidden: "" });
  var tagsPanel = el("div", { "class": "mdv-panel", role: "tabpanel", hidden: "" });
  sidebar.append(tabs, contentsPanel, filesPanel, linksPanel, tagsPanel, recentPanel);

  var KEYS = [
    ["[ / ]", "Back / forward (also Alt-\u2190 / Alt-\u2192)"],
    ["r", "Reload"],
    ["t", "Switch theme: auto, light, dark"],
    ["o", "Show or hide the contents"],
    hasFiles ? ["f", "Find a file"] : null,
    served ? ["/", "Search the text of every file"] : null,
    canEdit ? ["e", "Edit the source here (or double-click a paragraph)"] : null,
    canSource ? ["v", "View the Markdown source, highlighted (v again to go back)"] : null,
    ["s", "Present as slides (arrows to move, Esc to stop)"],
    ["+ / -", "Larger / smaller text (0 resets)"],
    ["c", "Copy the page (rich text, and Markdown as plain text)"],
    ["w", "Show or hide the word count and reading time"],
    served ? ["b", "Show backlinks: other files linking here"] : null,
    served ? ["#", "Show tags, and the files using each"] : null,
    hasRecent ? ["h", "Show recent files"] : null,
    served ? ["l", "Check this page's links"] : null,
    served ? ["D", "Show what changed since the last git commit"] : null,
    served ? ["B", "Blame: hover a paragraph to see who last changed it"] : null,
    ["j / k", "Scroll down / up"],
    ["d / u", "Half a page down / up"],
    ["g / G", "Top / bottom"],
    ["n / p", "Next / previous heading"],
    canSettings ? [",", "Settings"] : null,
    ["?", "Show this help"],
    ["Esc", "Close"],
  ].filter(Boolean);
  var overlay = el("div", { "class": "mdv-ui mdv-overlay", hidden: "" });
  var helpBox = el("div", { "class": "mdv-help", role: "dialog", "aria-label": "Keyboard shortcuts and about" },
    "<h2>Keyboard shortcuts</h2><table>" + KEYS.map(function (k) {
      return "<tr><td>" + k[0].split(" / ").map(function (x) { return "<kbd>" + x + "</kbd>"; }).join(" / ") +
        "</td><td>" + k[1] + "</td></tr>";
    }).join("") + "</table>");
  overlay.append(helpBox);

  // ---- About (in the help) -------------------------------------------------
  // The build comes with the page (data-about: nothing about this computer,
  // so it's fine in exports). Served pages also ask the viewer where its
  // files are. Built with text nodes only.
  var build = {};
  try { build = JSON.parse(script.dataset.about || "{}"); } catch (e) {}
  var aboutTable = el("table", { "class": "mdv-about" });
  var aboutLines = [];
  function aboutRow(label, value, href, into) {
    if (!value) return;
    var tr = el("tr");
    var th = el("td");
    th.textContent = label;
    var td = el("td");
    if (href) {
      var a = el("a", { href: href, target: "_blank", rel: "noopener" });
      a.textContent = value;
      td.append(a);
    } else {
      td.textContent = value;
    }
    tr.append(th, td);
    (into || aboutTable).append(tr);
    aboutLines.push(label + ": " + value);
  }
  var repo = build.repository || "";
  aboutRow("Version", build.version, repo && build.version ? repo + "/releases/tag/v" + build.version : "");
  if (build.commit) {
    aboutRow("Commit", build.commit.slice(0, 10) + (build.commitDate ? " (" + build.commitDate + ")" : ""),
      repo + "/commit/" + build.commit);
  }
  aboutRow("Built for", [build.target, build.profile].filter(Boolean).join(", "));
  aboutRow("Compiler", build.rustc);
  if (build.mermaid || build.katex) {
    aboutRow("Includes", ["Mermaid " + build.mermaid, "KaTeX " + build.katex].join(", "));
  }
  aboutRow("Page", served ? "served live by markdown-viewer" : siteRoot !== null ? "static site" : "standalone file");
  var aboutLocal = el("tbody");
  aboutTable.append(aboutLocal);
  var aboutLinks = el("p", { "class": "mdv-about-links" });
  [["Changelog", repo + "/blob/main/CHANGELOG.md"], ["Report an issue", repo + "/issues"],
    ["License: " + (build.license || "MIT"), repo + "/blob/main/LICENSE"]].forEach(function (l, i) {
    if (!repo) return;
    if (i) aboutLinks.append(document.createTextNode(" · "));
    var a = el("a", { href: l[1], target: "_blank", rel: "noopener" });
    a.textContent = l[0];
    aboutLinks.append(a);
  });
  var copyAbout = el("button", { type: "button", "class": "mdv-about-copy" }, "Copy details");
  copyAbout.addEventListener("click", function () {
    var text = "markdown-viewer\n" + aboutLines.join("\n") + "\nBrowser: " + navigator.userAgent;
    var done = function () { showToast("Copied the details"); };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, function () { fallbackCopy(text) && done(); });
    } else if (fallbackCopy(text)) {
      done();
    }
  });
  var aboutHead = el("h2");
  aboutHead.textContent = "About markdown-viewer";
  helpBox.append(aboutHead, aboutTable, aboutLinks, copyAbout);

  // ---- Word count ----------------------------------------------------------
  // The word count and reading time can be hidden to save toolbar space
  // (w, or click it); remembered like the theme.
  function showWordCount(show) {
    if (show) root.removeAttribute("data-mdv-stats");
    else root.setAttribute("data-mdv-stats", "off");
    savePrefs({ word_count: show });
  }
  function toggleWordCount() {
    var show = root.getAttribute("data-mdv-stats") === "off";
    showWordCount(show);
    showToast(show ? "Showing the word count" : "Word count hidden (w to show it)");
  }
  stats.addEventListener("click", toggleWordCount);

  // ---- Settings dialog -----------------------------------------------------
  // Edits the settings file. Each change is saved at once (the viewer
  // checks it, keeping the file's comments) and applied: rendering
  // changes arrive as live updates, page-level ones reload the page, and
  // the "Opening pages" ones are for the next launch.
  var SETTINGS = [
    { group: "Appearance" },
    { key: "theme", label: "Theme", type: "select", def: "auto",
      options: [["auto", "Auto (follow the system)"], ["light", "Light"], ["dark", "Dark"]] },
    { key: "style", label: "Page style", type: "select", def: "github",
      options: [["github", "GitHub"], ["wide", "Wide"], ["book", "Book (serif, for reading)"], ["compact", "Compact"]] },
    { key: "css", label: "Custom CSS file", type: "text", placeholder: "~/notes.css",
      help: "Applied after the built-in styles." },
    { key: "number_headings", label: "Number headings (1, 1.1, …)", type: "bool", def: false },
    { key: "line_numbers", label: "Number the lines of code blocks", type: "bool", def: false },
    { key: "source_highlighting", label: "Highlight the Markdown in the source view (v)", type: "bool", def: true,
      help: "Off shows it plain, which is quicker for very long files." },
    { key: "last_updated", label: "Show when the file last changed in git, with an Edit on GitHub link", type: "bool", def: true,
      help: "At the foot of the page." },
    { key: "word_count", label: "Show the word count and reading time", type: "bool", def: true,
      help: "w in the viewer shows or hides it too." },
    { key: "front_matter", label: "Front matter", type: "select", def: "table",
      options: [["table", "Show as a table"], ["hide", "Hide"]] },
    { group: "Markdown" },
    { key: "obsidian", label: "Obsidian notes", type: "select", def: null,
      options: [[null, "Automatic (in a vault)"], [true, "Always"], [false, "Never"]] },
    { key: "github_references", label: "Link #123, @user and commits to GitHub", type: "bool", def: true },
    { key: "github_repo", label: "GitHub repository", type: "text", placeholder: "owner/repo",
      detected: "github", help: "For those links, if it isn't the git remote's." },
    { group: "Editing" },
    { key: "editor", label: "Editor", type: "text", placeholder: "e.g. code -g {file}:{line}", detected: "editor",
      help: "For double-click to edit." },
    { group: "Opening pages", note: "Used the next time markdown-viewer opens a page." },
    { key: "open", label: "Open as", type: "select", def: "app",
      options: [["app", "App window (no tabs or address bar)"], ["window", "New window"], ["tab", "Tab"]] },
    { key: "browser", label: "Browser", type: "text", placeholder: "Default browser" },
    { key: "port", label: "Port", type: "number", placeholder: "Any free port" },
    { key: "exit_when_closed", label: "Stop the viewer when its window closes", type: "bool", def: true,
      help: "Otherwise it keeps running until Ctrl-C." },
    { group: "Other" },
    { key: "recent", label: "Remember opened files (the Recent page)", type: "bool", def: true },
    { key: "lint_ignore", label: "--lint rules to skip", type: "rules" },
  ];
  var settingsOverlay = el("div", { "class": "mdv-ui mdv-overlay", hidden: "" });
  var settingsBox = el("div", { "class": "mdv-help mdv-settings", role: "dialog", "aria-label": "Settings" });
  settingsOverlay.append(settingsBox);
  settingsOverlay.addEventListener("click", function (e) { if (e.target === settingsOverlay) settingsOverlay.hidden = true; });
  settingsBtn.addEventListener("click", openSettings);

  function openSettings() {
    // Added on first use, after the help (which stays the first overlay).
    if (!settingsOverlay.isConnected) document.body.append(settingsOverlay);
    settingsOverlay.hidden = false;
    settingsBox.textContent = "";
    var head = el("h2");
    head.textContent = "Settings";
    settingsBox.append(head, el("p", { "class": "mdv-empty" }, "Loading…"));
    fetch("/__mdv/settings").then(function (r) {
      if (!r.ok) throw new Error("error " + r.status);
      return r.json();
    }).then(buildSettings).catch(function (err) {
      settingsBox.lastChild.textContent = "Couldn't read the settings: " + err.message;
    });
  }

  function buildSettings(info) {
    settingsBox.textContent = "";
    var head = el("h2");
    head.textContent = "Settings";
    var where = el("p", { "class": "mdv-settings-file" });
    where.textContent = (info.exists ? "Saved in " : "Will be saved in ") + (info.path || "the settings file");
    settingsBox.append(head, where);
    if (info.error) {
      var bad = el("p", { "class": "mdv-settings-error" });
      bad.textContent = "The settings file has an error, so it can't be changed here: " + info.error;
      settingsBox.append(bad);
      return;
    }
    var values = info.values || {};
    var detected = info.detected || {};
    var overridden = info.overridden || [];
    var form = el("div", { "class": "mdv-settings-form" });
    settingsBox.append(form);
    SETTINGS.forEach(function (f) {
      if (f.group) {
        var g = el("h3");
        g.textContent = f.group;
        form.append(g);
        if (f.note) {
          var note = el("p", { "class": "mdv-settings-note" });
          note.textContent = f.note;
          form.append(note);
        }
        return;
      }
      form.append(settingRow(f, values[f.key], detected, overridden.indexOf(f.key) !== -1, info.lint_rules || []));
    });
  }

  function settingRow(f, value, detected, overridden, rules) {
    var row = el("div", { "class": "mdv-setting" });
    var id = "mdv-setting-" + f.key;
    var label = el("label", { "for": id });
    label.textContent = f.label;
    var status = el("span", { "class": "mdv-setting-status", "aria-live": "polite" });
    var control, read;
    if (f.type === "select") {
      control = el("select", { id: id });
      f.options.forEach(function (o, i) {
        var opt = el("option", { value: String(i) });
        opt.textContent = o[1] + (o[0] === f.def ? " (default)" : "");
        control.append(opt);
      });
      var current = value === undefined ? f.def : value;
      var index = f.options.findIndex(function (o) { return o[0] === current; });
      control.value = String(Math.max(index, 0));
      read = function () {
        var v = f.options[+control.value][0];
        return v === f.def ? null : v;
      };
    } else if (f.type === "bool") {
      control = el("input", { id: id, type: "checkbox" });
      control.checked = value === undefined ? f.def : !!value;
      read = function () { return control.checked === f.def ? null : control.checked; };
      row.classList.add("mdv-setting-check");
    } else if (f.type === "rules") {
      control = el("div", { "class": "mdv-setting-rules", id: id });
      var skipped = Array.isArray(value) ? value : [];
      rules.forEach(function (r) {
        var item = el("label", { title: r[1] });
        var box = el("input", { type: "checkbox", value: r[0] });
        box.checked = skipped.indexOf(r[0]) !== -1;
        var code = el("code");
        code.textContent = r[0];
        item.append(box, " ", code);
        control.append(item);
      });
      read = function () {
        var picked = Array.prototype.filter.call(control.querySelectorAll("input"), function (b) { return b.checked; })
          .map(function (b) { return b.value; });
        return picked.length ? picked : null;
      };
    } else {
      control = el("input", { id: id, type: f.type === "number" ? "number" : "text", spellcheck: "false" });
      if (f.type === "number") { control.min = "1"; control.max = "65535"; }
      control.value = value === undefined || value === null ? "" : String(value);
      var hint = f.detected && detected[f.detected] ? detected[f.detected] + " (found)" : f.placeholder;
      if (hint) control.placeholder = hint;
      read = function () {
        var v = control.value.trim();
        if (!v) return null;
        return f.type === "number" ? Number(v) : v;
      };
    }
    var saved = read();
    var save = function () {
      var next = read();
      if (JSON.stringify(next) === JSON.stringify(saved)) return;
      status.className = "mdv-setting-status";
      status.textContent = "Saving…";
      fetch("/__mdv/settings", {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-MDV": "1" },
        body: JSON.stringify({ key: f.key, value: next }),
      }).then(function (r) {
        if (!r.ok) return r.text().then(function (t) { throw new Error(t || "error " + r.status); });
        return r.json();
      }).then(function (res) {
        saved = next;
        status.textContent = overridden ? "Saved (a command-line option wins for now)"
          : f.key === "open" || f.key === "browser" || f.key === "port" || f.key === "recent" ||
            f.key === "lint_ignore" || f.key === "exit_when_closed" || f.key === "obsidian"
            ? "Saved, for next time" : "Saved";
        if (f.key === "theme") applyTheme(next || "auto");
        if (f.key === "word_count") showWordCount(next !== false);
        if (f.key === "source_highlighting" && sourceShown()) loadSource().catch(function () {});
        if (res.reload) {
          status.textContent = "Saved, reloading…";
          setTimeout(function () { location.reload(); }, 300);
        }
      }).catch(function (err) {
        status.className = "mdv-setting-status mdv-settings-error";
        status.textContent = err.message;
        restore(saved);
      });
    };
    // Put the control back to what's saved after a refused change.
    var restore = function (v) {
      if (f.type === "select") {
        var current = v === null ? f.def : v;
        control.value = String(Math.max(f.options.findIndex(function (o) { return o[0] === current; }), 0));
      } else if (f.type === "bool") {
        control.checked = v === null ? f.def : v;
      } else if (f.type === "rules") {
        control.querySelectorAll("input").forEach(function (b) { b.checked = !!v && v.indexOf(b.value) !== -1; });
      } else {
        control.value = v === null ? "" : String(v);
      }
    };
    control.addEventListener("change", save);
    if (control.tagName === "INPUT" && control.type !== "checkbox") {
      control.addEventListener("keydown", function (e) {
        if (e.key === "Enter") { e.preventDefault(); save(); }
        e.stopPropagation();
      });
    }
    var text = el("div", { "class": "mdv-setting-label" });
    text.append(label);
    if (f.help || overridden) {
      var help = el("div", { "class": "mdv-settings-note" });
      help.textContent = overridden ? "A command-line option sets this for this run." : f.help;
      text.append(help);
    }
    if (f.type === "bool") row.append(control, text, status);
    else row.append(text, control, status);
    return row;
  }

  // The theme setting also switches this page (and is remembered, as with t).
  function applyTheme(name) {
    root.setAttribute("data-mdv-theme", name);
    updateThemeButton();
    retheme();
    savePrefs({ theme: name });
  }

  // ---- Credits (in the help) -----------------------------------------------
  function creditList(items, extra) {
    var ul = el("ul", { "class": "mdv-credits" + (extra ? " " + extra : "") });
    items.forEach(function (c) {
      var li = el("li");
      var a = el("a", { href: c.url, target: "_blank", rel: "noopener" });
      a.textContent = c.name;
      li.append(a);
      if (c.version) {
        var version = el("span", { "class": "mdv-credit-version" });
        version.textContent = c.version;
        li.append(" ", version);
      }
      if (c.note) li.append(document.createTextNode(" — " + c.note));
      var lic = el("span", { "class": "mdv-credit-license" });
      lic.textContent = c.license || "";
      li.append(lic);
      ul.append(li);
    });
    return ul;
  }
  var credits = build.credits || {};
  if ((build.bundled && build.bundled.length) || (credits.crates && credits.crates.length)) {
    var creditsHead = el("h2");
    creditsHead.textContent = "Credits";
    var thanks = el("p", { "class": "mdv-credits-intro" });
    thanks.textContent = "markdown-viewer is built on the work of these projects. Thank you!";
    helpBox.append(creditsHead, thanks);
    if (build.bundled && build.bundled.length) {
      var inPages = el("h3");
      inPages.textContent = "In every page";
      helpBox.append(inPages, creditList(build.bundled));
    }
    if (credits.crates && credits.crates.length) {
      var builtWith = el("h3");
      builtWith.textContent = "Built with (Rust crates)";
      helpBox.append(builtWith, creditList(credits.crates, "mdv-credits-crates"));
      var others = (credits.total || 0) - credits.crates.length;
      if (others > 0) {
        var more = el("p", { "class": "mdv-credits-intro" });
        more.textContent = "…and " + others + " more crates they use (counting every platform). " +
          "Their licenses are on crates.io.";
        helpBox.append(more);
      }
    }
  }

  var localLoaded = false;
  function loadAboutLocal() {
    if (!served || localLoaded) return;
    localLoaded = true;
    fetch("/__mdv/about").then(function (r) { return r.json(); }).then(function (d) {
      var row = function (label, value) { aboutRow(label, value, "", aboutLocal); };
      row("System", d.os && d.arch ? d.os + ", " + d.arch : "");
      row("Serving", d.root);
      if (d.settings) row("Settings", d.settings[0] + (d.settings[1] ? "" : " (not created)"));
      row("State", d.state);
      row("Editor", d.editor || "none found");
      row("Custom CSS", d.custom_css);
      row("Obsidian", d.obsidian ? "on" : "");
      row("GitHub links", d.github);
    }).catch(function (err) { localLoaded = false; console.error("markdown-viewer:", err); });
  }
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
    if ((name === "files" && !hasFiles) || ((name === "links" || name === "tags") && !served) || (name === "recent" && !hasRecent) ||
        ["contents", "files", "links", "tags", "recent"].indexOf(name) === -1) name = "contents";
    currentTab = name;
    var panels = { contents: [contentsPanel, contentsTab], files: [filesPanel, filesTab], links: [linksPanel, linksTab], tags: [tagsPanel, tagsTab], recent: [recentPanel, recentTab] };
    for (var n in panels) {
      panels[n][0].hidden = n !== name;
      panels[n][1].setAttribute("aria-selected", String(n === name));
    }
    contentsBtn.setAttribute("aria-pressed", String(sidebarOpen() && name === "contents"));
    filesBtn.setAttribute("aria-pressed", String(sidebarOpen() && name === "files"));
    if (name === "files") loadFiles();
    if (name === "links") loadBacklinks();
    if (name === "tags") loadTags();
    if (name === "recent") loadRecent();
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
  helpBtn.addEventListener("click", function () { overlay.hidden = false; loadAboutLocal(); });
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
  // The files as a tree of folders, like an editor's file explorer.
  // Folders open and close with a click; which are open is kept for this
  // window's session. The folders above the current file start open, and
  // while filtering every folder with a match is shown open.
  var TREE_KEY = "mdv-tree-open";
  var openDirs = (function () {
    var set = {};
    try { (JSON.parse(sessionStorage.getItem(TREE_KEY)) || []).forEach(function (d) { set[d] = true; }); } catch (e) {}
    var parts = (currentDoc || "").split("/");
    for (var i = 1; i < parts.length; i++) set[parts.slice(0, i).join("/")] = true;
    return set;
  })();
  function saveOpenDirs() {
    try { sessionStorage.setItem(TREE_KEY, JSON.stringify(Object.keys(openDirs))); } catch (e) {}
  }
  function buildTree(paths) {
    var root = { dirs: {}, files: [] };
    paths.forEach(function (f) {
      var parts = f.split("/"), node = root;
      for (var i = 0; i < parts.length - 1; i++) {
        var path = parts.slice(0, i + 1).join("/");
        node = node.dirs[parts[i]] || (node.dirs[parts[i]] = { path: path, dirs: {}, files: [] });
      }
      node.files.push(f);
    });
    return root;
  }
  function byName(a, b) { return a.toLowerCase().localeCompare(b.toLowerCase()); }
  function fileIcon(f) {
    if (/\.(md|markdown|mdown|mkd|mkdn)$/i.test(f)) return "book";
    if (/\.(csv|tsv)$/i.test(f)) return "table";
    return "file";
  }
  function treeRow(tag, attrs, iconName, label) {
    var row = el(tag, attrs, icon(iconName));
    row.classList.add("mdv-tree-row");
    var name = el("span", { "class": "mdv-tree-name" });
    name.textContent = label;
    row.append(name);
    return row;
  }
  function renderTree(node, into, filtering) {
    Object.keys(node.dirs).sort(byName).forEach(function (name) {
      var dir = node.dirs[name], label = name;
      // A chain of folders holding only one folder shows as one row
      // (".github / workflows").
      while (!dir.files.length && Object.keys(dir.dirs).length === 1) {
        var only = Object.keys(dir.dirs)[0];
        label += " / " + only;
        dir = dir.dirs[only];
      }
      var open = filtering || !!openDirs[dir.path];
      var wrap = el("div", { "class": "mdv-tree-dir" });
      var row = treeRow("button", { type: "button", title: dir.path + "/", "aria-expanded": String(open) },
        open ? "file-directory-open-fill" : "file-directory", label);
      var kids = el("div", { "class": "mdv-tree-kids" });
      kids.hidden = !open;
      if (open) renderTree(dir, kids, filtering);
      row.addEventListener("click", function () {
        var nowOpen = kids.hidden;
        if (nowOpen) openDirs[dir.path] = true; else delete openDirs[dir.path];
        saveOpenDirs();
        kids.hidden = !nowOpen;
        row.setAttribute("aria-expanded", String(nowOpen));
        row.querySelector("svg").outerHTML = icon(nowOpen ? "file-directory-open-fill" : "file-directory");
        if (nowOpen && !kids.firstChild) renderTree(dir, kids, filtering);
      });
      wrap.append(row, kids);
      into.append(wrap);
    });
    node.files.slice().sort(function (a, b) {
      return byName(a.slice(a.lastIndexOf("/") + 1), b.slice(b.lastIndexOf("/") + 1));
    }).forEach(function (f) {
      var a = treeRow("a", { href: fileHref(f), title: f }, fileIcon(f), f.slice(f.lastIndexOf("/") + 1));
      if (f === currentDoc) a.classList.add("mdv-active");
      into.append(a);
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
    var tree = el("div", { "class": "mdv-tree" });
    renderTree(buildTree(shown), tree, !!q);
    fileList.append(tree);
    var active = tree.querySelector("a.mdv-active");
    if (active && !q) requestAnimationFrame(function () { active.scrollIntoView({ block: "nearest" }); });
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
  if ((initialSidebar === "files" && !hasFiles) || (initialSidebar === "links" && !served) ||
      (initialSidebar === "recent" && !hasRecent)) initialSidebar = "contents";
  if (["contents", "files", "links", "recent"].indexOf(initialSidebar) !== -1) {
    root.classList.add("mdv-sidebar-open");
    selectTab(initialSidebar);
  } else {
    selectTab("contents");
  }

  // ---- Copy buttons on code blocks ---------------------------------------
  function addCopyButtons(container) {
    container.querySelectorAll("pre").forEach(function (pre) {
      if (pre.classList.contains("mermaid") || pre.classList.contains("mdv-lib") || pre.hasAttribute("data-math-style")) return;
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

  // ---- Contents ([TOC]) ----------------------------------------------------
  // Folded parts open for printing, and fold again afterwards.
  var printOpened = [];
  window.addEventListener("beforeprint", function () {
    printOpened = Array.prototype.slice.call(body.querySelectorAll(".mdv-toc details:not([open])"));
    printOpened.forEach(function (d) { d.open = true; });
  });
  window.addEventListener("afterprint", function () {
    printOpened.forEach(function (d) { d.open = false; });
    printOpened = [];
  });

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
      case "e": if (!canEdit) return; if (sourceShown()) editLine(firstSourceLine()); else editAt(blockAtTop()); break;
      case "v": if (!canSource) return; showSource(!sourceShown()); break;
      case "s": startPresenting(); break;
      case "+": case "=": stepScale(1); break;
      case "-": case "_": stepScale(-1); break;
      case "0": setScale(1); break;
      case "c": copyPage(); break;
      case "b": if (!served) return; openSidebar("links"); break;
      case "#": if (!served) return; toggleSidebar("tags"); break;
      case "l": if (!served) return; checkLinks(); break;
      case "D": if (!served) return; toggleGit(); break;
      case "B": if (!served) return; toggleBlame(); break;
      case "j": window.scrollBy(0, 80); break;
      case "k": window.scrollBy(0, -80); break;
      case "d": window.scrollBy({ top: page / 2, behavior: "smooth" }); break;
      case "u": window.scrollBy({ top: -page / 2, behavior: "smooth" }); break;
      case "g": window.scrollTo({ top: 0, behavior: "smooth" }); break;
      case "G": window.scrollTo({ top: document.documentElement.scrollHeight, behavior: "smooth" }); break;
      case "n": jumpHeading(1); break;
      case "p": jumpHeading(-1); break;
      case "?": overlay.hidden = !overlay.hidden; if (!overlay.hidden) loadAboutLocal(); break;
      case ",": if (!canSettings) return; openSettings(); break;
      case "w": toggleWordCount(); break;
      case "h": if (!hasRecent) return; toggleSidebar("recent"); break;
      case "[": history.back(); break;
      case "]": history.forward(); break;
      case "r": location.reload(); break;
      case "Escape":
        if (!overlay.hidden) overlay.hidden = true;
        else if (!settingsOverlay.hidden) settingsOverlay.hidden = true;
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
    if (blameOn) loadBlame(false);
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
  function blockAtTop(offset) {
    var y = offset === undefined ? 80 : offset, found = null;
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
  function editAt(node) { editLine(lineOf(node) || 1); }
  function editLine(line) {
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
    if (t.closest("a, button, input, .mdv-ui, img, div.mermaid, .mdv-map, .mdv-stl, .mdv-diagram")) return;
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
  // And once the page's own drawings are in (their scripts come after
  // this one, so wait for the page to finish loading them).
  document.addEventListener("DOMContentLoaded", function () {
    Promise.all(window.mdvInitial || []).then(function () {
      if (!userScrolled) restorePosition();
    });
  });
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
    var diagram = e.target.closest("div.mermaid, .mdv-map:not(.mdv-map-invalid), " +
      ".mdv-diagram:not(.mdv-diagram-invalid, .mdv-diagram-missing, .mdv-qr, .mdv-chess, .mdv-molecule-3d)");
    if (diagram && diagram.querySelector("svg")) {
      var svg = diagram.querySelector("svg").cloneNode(true);
      var vb = svg.viewBox && svg.viewBox.baseVal;
      svg.removeAttribute("style");
      svg.removeAttribute("width");
      svg.removeAttribute("height");
      svg.style.width = Math.min(window.innerWidth * 0.92, (vb && vb.width ? vb.width : 800) * 1.6) + "px";
      svg.style.height = "auto";
      // The page's diagram styles apply inside .markdown-body.
      var wrap = el("div", { "class": "markdown-body mdv-zoom-body" });
      var copy = el(diagram.tagName.toLowerCase(), { "class": diagram.className });
      copy.append(svg);
      wrap.append(copy);
      openZoom(wrap);
    }
  });

  // ---- Turning STL models -------------------------------------------------------
  // The page draws each model from one angle (stl.rs); dragging turns it,
  // redrawing its triangles from the vertices in data-mdv-mesh with the
  // same maths as stl.rs. Double-click goes back to the first view.
  var STL_YAW = -0.6, STL_PITCH = 1.0;
  var stlState = new WeakMap();
  function stlModel(fig) {
    var m = stlState.get(fig);
    if (!m) {
      var n = fig.getAttribute("data-mdv-mesh").split(",").map(Number);
      m = { mesh: n, yaw: STL_YAW, pitch: STL_PITCH, polys: fig.querySelectorAll("polygon") };
      stlState.set(fig, m);
    }
    return m;
  }
  function drawStl(m) {
    var cy = Math.cos(m.yaw), sy = Math.sin(m.yaw), cp = Math.cos(m.pitch), sp = Math.sin(m.pitch);
    var v = m.mesh, faces = [], half = 200, scale = half * 0.92;
    for (var i = 0; i + 8 < v.length; i += 9) {
      var p = [];
      for (var k = 0; k < 3; k++) {
        var x0 = v[i + k * 3], y0 = v[i + k * 3 + 1], z0 = v[i + k * 3 + 2];
        var x = x0 * cy - y0 * sy, y = x0 * sy + y0 * cy;
        p.push([x, -(y * cp + z0 * sp), -y * sp + z0 * cp]);
      }
      var a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
      var b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
      var nx = a[1] * b[2] - a[2] * b[1], ny = a[2] * b[0] - a[0] * b[2], nz = a[0] * b[1] - a[1] * b[0];
      var len = Math.max(Math.sqrt(nx * nx + ny * ny + nz * nz), 1e-12);
      var dot = Math.abs((nx * -0.4 + ny * -0.5 + nz * 0.77) / len);
      faces.push({
        pts: p.map(function (q) { return (half + q[0] * scale).toFixed(1) + "," + (half + q[1] * scale).toFixed(1); }).join(" "),
        depth: (p[0][2] + p[1][2] + p[2][2]) / 3,
        light: 0.3 + 0.7 * dot,
      });
    }
    faces.sort(function (f, g) { return f.depth - g.depth; });
    faces.forEach(function (f, j) {
      var poly = m.polys[j];
      if (!poly) return;
      poly.setAttribute("points", f.pts);
      poly.style.setProperty("--l", f.light.toFixed(2));
    });
  }
  // 3D molecules (molecules.rs): balls and sticks, redrawn from
  // data-mdv-atoms ("x,y,z,radius,element;…") and data-mdv-bonds ("a,b;…")
  // the way molecules.rs `scene` draws them.
  function moleculeModel(fig) {
    var m = stlState.get(fig);
    if (!m) {
      var atoms = fig.getAttribute("data-mdv-atoms").split(";").map(function (a) {
        var f = a.split(",");
        return { v: [+f[0], +f[1], +f[2]], r: +f[3], el: f[4] };
      });
      var bondText = fig.getAttribute("data-mdv-bonds");
      var bonds = bondText ? bondText.split(";").map(function (b) { return b.split(",").map(Number); }) : [];
      m = { atoms: atoms, bonds: bonds, yaw: STL_YAW, pitch: STL_PITCH, svg: fig.querySelector("svg"), molecule: true };
      stlState.set(fig, m);
    }
    return m;
  }
  function drawMolecule(m) {
    var cy = Math.cos(m.yaw), sy = Math.sin(m.yaw), cp = Math.cos(m.pitch), sp = Math.sin(m.pitch);
    var half = 200, scale = half * 0.92;
    var p = m.atoms.map(function (a) {
      var x = a.v[0] * cy - a.v[1] * sy, y = a.v[0] * sy + a.v[1] * cy, z = a.v[2];
      return [x, -(y * cp + z * sp), -y * sp + z * cp];
    });
    var items = [];
    m.bonds.forEach(function (b, i) { items.push([(p[b[0]][2] + p[b[1]][2]) / 2 - 0.01, 0, i]); });
    m.atoms.forEach(function (a, i) { items.push([p[i][2], 1, i]); });
    items.sort(function (a, b) { return a[0] - b[0]; });
    var f = function (n) { return (half + n * scale).toFixed(1); };
    m.svg.innerHTML = items.map(function (it) {
      if (it[1] === 0) {
        var b = m.bonds[it[2]];
        return '<line class="mdv-mol-stick" x1="' + f(p[b[0]][0]) + '" y1="' + f(p[b[0]][1]) +
          '" x2="' + f(p[b[1]][0]) + '" y2="' + f(p[b[1]][1]) + '"/>';
      }
      var a = m.atoms[it[2]];
      return '<circle class="mdv-el-' + a.el + '" cx="' + f(p[it[2]][0]) + '" cy="' + f(p[it[2]][1]) +
        '" r="' + Math.max(a.r * scale, 1).toFixed(1) + '"/>';
    }).join("");
  }
  var TURNABLE = ".markdown-body .mdv-stl[data-mdv-mesh], .markdown-body .mdv-molecule[data-mdv-atoms]";
  function turnable(fig) { return fig.hasAttribute("data-mdv-atoms") ? moleculeModel(fig) : stlModel(fig); }
  function drawTurnable(m) { if (m.molecule) drawMolecule(m); else drawStl(m); }
  var stlDrag = null;
  document.addEventListener("pointerdown", function (e) {
    var fig = e.target.closest && e.target.closest(TURNABLE);
    if (!fig || e.button !== 0) return;
    e.preventDefault();
    stlDrag = { model: turnable(fig), x: e.clientX, y: e.clientY, frame: 0, svg: fig.querySelector("svg") };
    stlDrag.svg.style.cursor = "grabbing";
  });
  document.addEventListener("pointermove", function (e) {
    if (!stlDrag) return;
    var d = stlDrag;
    d.model.yaw += (e.clientX - d.x) * 0.01;
    d.model.pitch += (e.clientY - d.y) * 0.01;
    d.x = e.clientX;
    d.y = e.clientY;
    if (!d.frame) d.frame = requestAnimationFrame(function () { d.frame = 0; drawTurnable(d.model); });
  });
  document.addEventListener("pointerup", function () {
    if (stlDrag) stlDrag.svg.style.cursor = "";
    stlDrag = null;
  });
  document.addEventListener("dblclick", function (e) {
    var fig = e.target.closest && e.target.closest(TURNABLE);
    if (!fig) return;
    var m = turnable(fig);
    m.yaw = STL_YAW;
    m.pitch = STL_PITCH;
    drawTurnable(m);
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

  // ---- Links out of the viewer -----------------------------------------------
  // Links to web pages, and to local files the viewer doesn't show as pages
  // (images, PDFs, anything not Markdown or CSV), open in a new browser
  // window, so the document stays where it was. Links between documents,
  // and to places on this page, stay in this window. The target is set
  // when the link is followed, so live updates need nothing extra.
  var VIEWABLE = /\.(md|markdown|mdown|mkd|mkdn|csv|tsv)$/i;
  function opensHere(url) {
    if (url.origin === location.origin && url.pathname === location.pathname &&
        url.search === location.search) return true; // this page (#anchors)
    if (url.protocol !== "http:" && url.protocol !== "https:" && url.protocol !== "file:") return true; // mailto: and the like
    if (siteRoot !== null) {
      // An exported site: its pages are .html files (or folders) under its root.
      var base = new URL(siteRoot || "./", location.href).href;
      return url.href.indexOf(base) === 0 && /(\/|\.html)$/.test(url.pathname);
    }
    if (served) {
      if (url.origin !== location.origin || url.pathname.indexOf("/__mdv/") === 0) return false;
      var last = url.pathname.slice(url.pathname.lastIndexOf("/") + 1);
      // A document, or a folder (no extension; the viewer lists it).
      return VIEWABLE.test(decodeURIComponent(last)) || last.indexOf(".") === -1;
    }
    // A standalone page: only web addresses leave.
    return url.protocol === "file:";
  }
  document.addEventListener("click", function (e) {
    var a = e.target.closest && e.target.closest("a[href]");
    // In the document or a presentation slide, not the viewer's own UI.
    if (!a || !a.closest(".markdown-body, .mdv-slide")) return;
    var url;
    try { url = new URL(a.href, location.href); } catch (err) { return; }
    if (opensHere(url)) return;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
  }, true);

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
  document.addEventListener("mdv:goto", function (e) {
    if (sourceShown()) showSource(false);
    revealLine(e.detail.line);
  });

  // ---- Source view -----------------------------------------------------------
  // The file's Markdown, highlighted by the viewer, in place of the page.
  // Switching keeps your place: the source opens at the line of the block
  // at the top, and going back shows the block for the line at the top.
  var sourceView = el("div", { "class": "markdown-body mdv-source-view" });
  body.after(sourceView);
  function sourceShown() { return root.getAttribute("data-mdv-view") === "source"; }
  // Two steps, so the view is instant: the plain source (fast), then the
  // highlighted one (slow for long files) swapped in, keeping the scroll.
  // `sourceSeq` drops answers that arrive after a newer request.
  var sourceSeq = 0;
  var sourceStatus = el("div", { "class": "mdv-ui mdv-source-status", role: "status" });
  function fetchSource(plain) {
    return fetch("/__mdv/source-html?doc=" + encodeURIComponent(currentDoc) + (plain ? "&plain=1" : ""))
      .then(function (r) {
        if (!r.ok) throw new Error("error " + r.status);
        return r.text().then(function (html) {
          return { html: html, highlighted: r.headers.get("X-MDV-Highlighted") === "true" };
        });
      });
  }
  function busy(text) {
    sourceStatus.textContent = text || "";
    sourceBtn.classList.toggle("mdv-busy", !!text);
    if (text && !sourceStatus.isConnected) document.body.append(sourceStatus);
    sourceStatus.hidden = !text;
  }
  // Load (or reload) the source; `atLine` scrolls there after the first step.
  function loadSource(atLine) {
    var seq = ++sourceSeq;
    var first = atLine ? fetchSource(true).then(function (res) {
      if (seq !== sourceSeq) return;
      sourceView.innerHTML = res.html;
      scrollToSourceLine(atLine);
    }) : Promise.resolve();
    return first.then(function () {
      if (seq !== sourceSeq) return;
      busy("Highlighting…");
      return fetchSource(false).then(function (res) {
        if (seq !== sourceSeq) return;
        // Same text and line heights, so the scroll position carries over.
        var y = window.scrollY;
        sourceView.innerHTML = res.html;
        window.scrollTo(0, y);
      });
    }).finally(function () { if (seq === sourceSeq) busy(""); });
  }
  function scrollToSourceLine(line) {
    var target = sourceView.querySelector('.mdv-line[data-line="' + line + '"]');
    if (target) scrollToTop(target); else window.scrollTo(0, 0);
  }
  function sourceLines() { return sourceView.querySelectorAll(".mdv-line"); }
  // The first source line at the top of the window (binary search: files
  // can be long).
  function firstSourceLine() {
    var lines = sourceLines(), lo = 0, hi = lines.length - 1, y = 20;
    if (!lines.length) return 1;
    while (lo < hi) {
      var mid = (lo + hi) >> 1;
      if (lines[mid].getBoundingClientRect().bottom <= y) lo = mid + 1; else hi = mid;
    }
    return +lines[lo].getAttribute("data-line") || 1;
  }
  function scrollToTop(el) {
    window.scrollTo(0, Math.max(0, el.getBoundingClientRect().top + window.scrollY - 16));
  }
  function showSource(on) {
    if (on) {
      // The block right at the top of the window (not under the toolbar
      // margin), so the two views line up.
      var line = lineOf(blockAtTop(20)) || 1;
      // Switch at once, with feedback, rather than after loading.
      sourceView.replaceChildren(el("p", { "class": "mdv-source-loading" }, "Loading the source…"));
      root.setAttribute("data-mdv-view", "source");
      sourceBtn.setAttribute("aria-pressed", "true");
      window.scrollTo(0, 0);
      busy("Loading the source…");
      loadSource(line).catch(function (err) {
        console.error("markdown-viewer:", err);
        if (sourceShown() && !sourceView.querySelector(".mdv-line")) showSource(false);
        showToast("Couldn't load the source");
      });
    } else {
      ++sourceSeq;
      busy("");
      var at = firstSourceLine();
      root.removeAttribute("data-mdv-view");
      sourceBtn.setAttribute("aria-pressed", "false");
      var block = blockForLine(at);
      if (block) scrollToTop(block); else window.scrollTo(0, 0);
    }
  }
  sourceBtn.addEventListener("click", function () { showSource(!sourceShown()); });
  // Keep the source current as the file changes.
  document.addEventListener("mdv:updated", function () {
    if (sourceShown()) loadSource().catch(function () {});
  });
  // Double-click a line to edit the file there.
  sourceView.addEventListener("dblclick", function (e) {
    if (!canEdit) return;
    var line = e.target.closest && e.target.closest(".mdv-line");
    if (!line) return;
    var sel = window.getSelection && window.getSelection();
    if (sel) sel.removeAllRanges();
    editLine(+line.getAttribute("data-line") || 1);
  });

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

  // ---- Blame: who last changed a block ----------------------------------------
  // With B on, resting the pointer on a paragraph shows its latest change
  // (the newest commit among its lines) beside it.
  var blameOn = false, blame = null, blameFor = null, blameShow, blameHide;
  // The block under the pointer, shown as soon as blame arrives (git can
  // take a moment, and the pointer may already be resting there).
  var blamePointer = null;
  var blameBox = el("div", { "class": "mdv-ui mdv-blame", role: "tooltip", hidden: "" });
  document.body.append(blameBox);
  function loadBlame(announce) {
    return fetch("/__mdv/blame?doc=" + encodeURIComponent(currentDoc)).then(function (r) { return r.json(); }).then(function (d) {
      blame = d;
      if (blameOn && blamePointer && blamePointer.isConnected) {
        blameFor = null;
        showBlame(blamePointer);
      }
      if (!announce) return;
      if (d.status === "not-a-repo") showToast("Not in a git repository");
      else if (d.status === "untracked") showToast("Not committed yet");
      else showToast("Blame: rest the pointer on a paragraph (B to stop)");
    }).catch(function () { if (announce) showToast("Couldn't run git blame"); });
  }
  function toggleBlame() {
    blameOn = !blameOn;
    root.toggleAttribute("data-mdv-blame", blameOn);
    if (blameOn) loadBlame(true);
    else { hideBlame(); showToast("Blame off"); }
  }
  function hideBlame() {
    clearTimeout(blameShow);
    blameBox.hidden = true;
    blameFor = null;
    blamePointer = null;
  }
  function blockFor(node) {
    var n = node && node.closest && node.closest("[data-sourcepos]");
    while (n && INLINE.test(n.tagName)) n = n.parentElement && n.parentElement.closest("[data-sourcepos]");
    return n && body.contains(n) ? n : null;
  }
  function latestChange(node) {
    var r = range(node);
    if (!r || !blame || blame.status !== "ok") return null;
    var best = null;
    for (var line = r[0]; line <= r[1] && line <= blame.lines.length; line++) {
      var c = blame.commits[blame.lines[line - 1]];
      // Not committed yet counts as newest.
      if (c && (!best || (!c.hash && best.hash) || (!!c.hash === !!best.hash && c.time > best.time))) best = c;
    }
    return best;
  }
  function showBlame(node) {
    var c = latestChange(node);
    if (!c) { blameBox.hidden = true; blameFor = null; return; }
    blameFor = node;
    blameBox.textContent = "";
    var who = el("b");
    who.textContent = c.author;
    var when = el("span", { "class": "mdv-blame-date" });
    when.textContent = new Date(c.date + "T12:00:00").toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" });
    var what = el("span", { "class": "mdv-blame-summary" });
    what.textContent = c.summary;
    blameBox.append(who, " · ", when, el("br"), what);
    if (c.hash) {
      var link = el(c.url ? "a" : "code", c.url ? { href: c.url, target: "_blank", rel: "noopener" } : {});
      link.textContent = c.hash;
      blameBox.append(" ", link);
    }
    blameBox.hidden = false;
    var r = node.getBoundingClientRect();
    var article = body.getBoundingClientRect();
    var w = blameBox.offsetWidth, h = blameBox.offsetHeight;
    var left, top;
    if (window.innerWidth - article.right >= w + 16) {
      // Beside the page, level with the block.
      left = article.right + 12;
      top = r.top;
    } else {
      left = Math.max(8, Math.min(r.right - w, window.innerWidth - w - 8));
      top = r.top - h - 4;
    }
    blameBox.style.left = left + window.scrollX + "px";
    blameBox.style.top = Math.max(0, top + window.scrollY) + "px";
  }
  body.addEventListener("mouseover", function (e) {
    if (!blameOn) return;
    var node = blockFor(e.target);
    if (!node || node === blameFor) return;
    blamePointer = node;
    clearTimeout(blameShow);
    clearTimeout(blameHide);
    blameShow = setTimeout(function () { showBlame(node); }, 150);
  });
  body.addEventListener("mouseleave", function () {
    if (!blameOn) return;
    clearTimeout(blameHide);
    blameHide = setTimeout(hideBlame, 400);
  });
  blameBox.addEventListener("mouseenter", function () { clearTimeout(blameHide); });
  blameBox.addEventListener("mouseleave", function () { blameHide = setTimeout(hideBlame, 400); });

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

  // ---- Tags -----------------------------------------------------------------------
  // Every tag in the served folder (front matter tags, and #tags in
  // Obsidian notes), each opening to the files that use it. The page's own
  // tags come first. Clicking a tag chip in the page opens it here.
  var openTagName = null;
  function tagChip(name, label) {
    var c = el("span", { "class": "mdv-tag", "data-mdv-tag": name });
    c.textContent = "#" + label;
    return c;
  }
  function loadTags() {
    fetch("/__mdv/tags").then(function (r) { return r.json(); }).then(function (data) {
      var tags = data.tags || [];
      tagsPanel.textContent = "";
      if (!tags.length) {
        tagsPanel.append(el("div", { "class": "mdv-empty" }, "No tags in this folder"));
        return;
      }
      var find = el("input", { "class": "mdv-filter", type: "search", placeholder: "Find tags\u2026", "aria-label": "Find tags" });
      tagsPanel.append(find);
      // This page's tags, from its chips.
      var here = {};
      body.querySelectorAll(".mdv-tag[data-mdv-tag]").forEach(function (c) { here[c.getAttribute("data-mdv-tag")] = true; });
      var mine = tags.filter(function (t) { return here[t.tag.toLowerCase()]; });
      if (mine.length) {
        var title = el("div", { "class": "mdv-dir" });
        title.textContent = "On this page";
        var chips = el("div", { "class": "mdv-tag-chips" });
        mine.forEach(function (t) { chips.append(tagChip(t.tag.toLowerCase(), t.tag)); });
        tagsPanel.append(title, chips);
      }
      var allTitle = el("div", { "class": "mdv-dir" });
      allTitle.textContent = tags.length + (tags.length === 1 ? " tag" : " tags");
      tagsPanel.append(allTitle);
      var groups = [];
      tags.forEach(function (t) {
        var key = t.tag.toLowerCase();
        var group = el("details", { "class": "mdv-tag-group", "data-mdv-tag": key });
        var summary = el("summary");
        var count = el("span", { "class": "mdv-tag-count" });
        count.textContent = String(t.files.length);
        summary.append(tagChip(key, t.tag), count);
        group.append(summary);
        t.files.forEach(function (f) {
          var a = el("a", {
            "class": "mdv-hit" + (f.file === currentDoc ? " mdv-current" : ""),
            href: "/" + f.file.split("/").map(encodeURIComponent).join("/") + "#mdv-line-" + f.line,
            title: f.file + ":" + f.line,
          });
          var where = el("span", { "class": "mdv-hit-file" });
          where.textContent = f.file + (f.count > 1 ? " (" + f.count + "\u00d7)" : "");
          a.append(where);
          group.append(a);
        });
        groups.push([key, group]);
        tagsPanel.append(group);
      });
      find.addEventListener("input", function () {
        var q = find.value.trim().replace(/^#/, "").toLowerCase();
        groups.forEach(function (g) { g[1].hidden = q !== "" && g[0].indexOf(q) === -1; });
      });
      if (openTagName) showTag(openTagName);
    }).catch(function () {
      tagsPanel.textContent = "";
      tagsPanel.append(el("div", { "class": "mdv-empty" }, "Couldn't load tags"));
    });
  }
  function showTag(name) {
    var groups = tagsPanel.querySelectorAll("details.mdv-tag-group");
    for (var i = 0; i < groups.length; i++) {
      if (groups[i].getAttribute("data-mdv-tag") === name) {
        groups[i].open = true;
        groups[i].scrollIntoView({ block: "nearest" });
        openTagName = null;
        return;
      }
    }
  }
  tagsTab.addEventListener("click", function () { openSidebar("tags"); });
  // A chip (in the page, or "On this page") opens its tag.
  document.addEventListener("click", function (e) {
    var chip = e.target.closest && e.target.closest(".mdv-tag[data-mdv-tag]");
    if (!chip || !served || chip.closest("summary")) return;
    e.preventDefault();
    openTagName = chip.getAttribute("data-mdv-tag");
    if (sidebarOpen() && currentTab === "tags" && tagsPanel.querySelector("details")) showTag(openTagName);
    else openSidebar("tags");
  });
  document.addEventListener("mdv:updated", function () {
    if (sidebarOpen() && currentTab === "tags") loadTags();
  });

  // ---- Recent files -------------------------------------------------------------
  // The sidebar's Recent tab, and the Recent page's rows. Items inside the
  // served folder are links; others ask the viewer to open them (in a
  // viewer of their own). Removing forgets an item; the Recent page also
  // updates itself from the server.
  var RECENT_ICONS = { folder: "file-directory", markdown: "book", table: "table", file: "file" };
  function postRecent(action, path) {
    return fetch("/__mdv/recent/" + action, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-MDV": "1" },
      body: JSON.stringify({ path: path }),
    }).then(function (r) { if (!r.ok) throw new Error("error " + r.status); });
  }
  function openRecent(path) {
    postRecent("open", path).then(function () { showToast("Opening in a new window\u2026"); },
      function () { showToast("Couldn't open it"); });
  }
  function removeRecent(path, row) {
    postRecent("remove", path).then(function () {
      var list = row.parentElement;
      row.remove();
      // Drop an emptied group (on the Recent page) and its title.
      if (list && list.classList.contains("mdv-recent-list") && !list.children.length) {
        var title = list.previousElementSibling;
        if (title && title.classList.contains("mdv-recent-group")) title.remove();
        list.remove();
      }
      if (currentTab === "recent" && !recentPanel.querySelector(".mdv-recent-row")) loadRecent();
    }, function () { showToast("Couldn't remove it"); });
  }
  function loadRecent() {
    if (!hasRecent) return;
    fetch("/__mdv/recent").then(function (r) { return r.json(); }).then(function (data) {
      recentPanel.textContent = "";
      var items = data.items || [];
      if (!items.length) {
        recentPanel.append(el("div", { "class": "mdv-empty" }, "Nothing opened yet"));
        return;
      }
      var group = null;
      items.forEach(function (item) {
        if (item.group !== group) {
          group = item.group;
          var heading = el("div", { "class": "mdv-dir" });
          heading.textContent = group;
          recentPanel.append(heading);
        }
        var row = el("div", { "class": "mdv-recent-row" });
        var link = el("a", { "class": "mdv-tree-row", href: item.href || "#", title: item.path }, icon(RECENT_ICONS[item.kind] || "file"));
        var text = el("span", { "class": "mdv-recent-text" });
        var name = el("span", { "class": "mdv-tree-name" });
        name.textContent = item.kind === "folder" ? item.name + "/" : item.name;
        var sub = el("span", { "class": "mdv-recent-sub" });
        var where = el("span", { "class": "mdv-recent-folder" });
        where.textContent = item.folder;
        var when = el("span", { "class": "mdv-recent-ago" });
        when.textContent = "\u00b7 " + item.ago;
        sub.append(where, when);
        text.append(name, sub);
        link.append(text);
        if (item.href && item.href.split("#")[0] === location.pathname) link.classList.add("mdv-active");
        if (!item.href) link.addEventListener("click", function (e) { e.preventDefault(); openRecent(item.path); });
        var remove = el("button", { type: "button", "class": "mdv-recent-remove", title: "Remove from Recent",
          "aria-label": "Remove " + item.name + " from Recent" }, icon("x"));
        remove.addEventListener("click", function () { removeRecent(item.path, row); });
        row.append(link, remove);
        recentPanel.append(row);
      });
    }).catch(function () {
      recentPanel.textContent = "";
      recentPanel.append(el("div", { "class": "mdv-empty" }, "Couldn't load recent files"));
    });
  }
  recentTab.addEventListener("click", function () { openSidebar("recent"); });
  // The Recent page's own rows (it's re-rendered by live updates, so
  // listen on the document).
  document.addEventListener("click", function (e) {
    var item = e.target.closest && e.target.closest(".markdown-body .mdv-recent-item");
    if (!item) return;
    var path = item.getAttribute("data-path");
    if (e.target.closest(".mdv-recent-remove")) {
      e.preventDefault();
      removeRecent(path, item);
    } else if (e.target.closest("a[data-mdv-recent-open]")) {
      e.preventDefault();
      openRecent(path);
    }
  });

  addCopyButtons(body);
  buildContents();
  updateStats();
})();
