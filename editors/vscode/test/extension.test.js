// Tests the VS Code extension without VS Code: a stand-in `vscode` module
// records what the extension registers, and child_process.spawn is
// replaced to record the markdown-viewer commands it runs.
// Run with: node editors/vscode/test/extension.test.js
"use strict";

const assert = require("assert");
const Module = require("module");
const childProcess = require("child_process");

const spawned = [];
childProcess.spawn = (command, args, options) => {
  spawned.push({ command, args, options });
  return { on() {}, unref() {} };
};

const handlers = {};
const commands = {};
let settings = { path: "markdown-viewer", followCursor: true };
const vscodeStub = {
  workspace: {
    getConfiguration: () => ({
      get: (key) => settings[key],
      update: async (key, value) => { settings[key] = value; },
    }),
  },
  window: {
    activeTextEditor: null,
    showErrorMessage: () => {},
    showInformationMessage: () => {},
    onDidChangeTextEditorSelection: (fn) => { handlers.selection = fn; return { dispose() {} }; },
  },
  commands: {
    registerCommand: (name, fn) => { commands[name] = fn; return { dispose() {} }; },
  },
  ConfigurationTarget: { Global: 1 },
};
const load = Module._load;
Module._load = function (request, ...rest) {
  return request === "vscode" ? vscodeStub : load.call(this, request, ...rest);
};

const extension = require("../extension.js");
extension.activate({ subscriptions: [] });

function editor(file, line, languageId = "markdown") {
  return {
    document: { languageId, uri: { scheme: "file", fsPath: file } },
    selection: { active: { line: line - 1 } },
  };
}

(async () => {
  // "Show This Line" opens or scrolls the preview at the cursor.
  vscodeStub.window.activeTextEditor = editor("/notes/a b.md", 12);
  commands["markdownViewer.showLine"]();
  assert.deepStrictEqual(spawned.pop().args, ["--goto", "/notes/a b.md:12", "--exit-when-closed"]);

  // Following the cursor: debounced, and only if a preview is running.
  handlers.selection({ textEditor: editor("/notes/a b.md", 20) });
  handlers.selection({ textEditor: editor("/notes/a b.md", 21) });
  await new Promise((r) => setTimeout(r, 400));
  assert.strictEqual(spawned.length, 1, "one command after the cursor settles");
  assert.deepStrictEqual(spawned.pop().args, [
    "--goto", "/notes/a b.md:21", "--exit-when-closed", "--if-running",
  ]);

  // Not for other languages, or when following is off.
  handlers.selection({ textEditor: editor("/src/main.rs", 3, "rust") });
  await commands["markdownViewer.toggleFollow"]();
  handlers.selection({ textEditor: editor("/notes/a b.md", 5) });
  await new Promise((r) => setTimeout(r, 400));
  assert.strictEqual(spawned.length, 0);

  // A custom command path is used.
  settings.path = "/opt/bin/markdown-viewer";
  commands["markdownViewer.showLine"]();
  assert.strictEqual(spawned.pop().command, "/opt/bin/markdown-viewer");

  console.log("vscode extension: all tests passed");
})().catch((err) => {
  console.error(err);
  process.exit(1);
});
