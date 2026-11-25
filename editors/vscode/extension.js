// markdown-viewer for VS Code: "Show This Line in Preview" opens (or
// scrolls) markdown-viewer at the cursor, and while a preview is open it
// follows the cursor. Everything goes through the markdown-viewer command:
//   markdown-viewer --goto FILE:LINE [--if-running] --exit-when-closed
"use strict";

const vscode = require("vscode");
const { spawn } = require("child_process");

const FOLLOW_DELAY_MS = 250;

function config() {
  return vscode.workspace.getConfiguration("markdownViewer");
}

/** Run markdown-viewer detached, so a preview it starts outlives the call. */
function run(args) {
  const command = config().get("path") || "markdown-viewer";
  let child;
  try {
    child = spawn(command, args, { detached: true, stdio: "ignore" });
  } catch (err) {
    report(err, command);
    return;
  }
  child.on("error", (err) => report(err, command));
  child.unref();
}

function report(err, command) {
  const hint = err && err.code === "ENOENT"
    ? `Couldn't find "${command}". Install markdown-viewer, or set markdownViewer.path.`
    : `markdown-viewer failed: ${err && err.message}`;
  vscode.window.showErrorMessage(hint);
}

function isMarkdown(editor) {
  return !!editor && editor.document.languageId === "markdown" && editor.document.uri.scheme === "file";
}

/** `--goto` arguments for the cursor's line. */
function gotoArgs(editor, onlyIfRunning) {
  const file = editor.document.uri.fsPath;
  const line = editor.selection.active.line + 1;
  const args = ["--goto", `${file}:${line}`, "--exit-when-closed"];
  if (onlyIfRunning) args.push("--if-running");
  return args;
}

function activate(context) {
  let timer;

  context.subscriptions.push(
    vscode.commands.registerCommand("markdownViewer.showLine", () => {
      const editor = vscode.window.activeTextEditor;
      if (!isMarkdown(editor)) {
        vscode.window.showInformationMessage("Open a Markdown file to preview it.");
        return;
      }
      run(gotoArgs(editor, false));
    }),

    vscode.commands.registerCommand("markdownViewer.toggleFollow", async () => {
      const follow = !config().get("followCursor");
      await config().update("followCursor", follow, vscode.ConfigurationTarget.Global);
      vscode.window.showInformationMessage(
        follow ? "markdown-viewer: following the cursor." : "markdown-viewer: not following the cursor.",
      );
    }),

    // Follow the cursor, but never open a preview that isn't already open
    // (--if-running), so closing the preview window stops following.
    vscode.window.onDidChangeTextEditorSelection((event) => {
      if (!config().get("followCursor") || !isMarkdown(event.textEditor)) return;
      clearTimeout(timer);
      timer = setTimeout(() => run(gotoArgs(event.textEditor, true)), FOLLOW_DELAY_MS);
    }),

    { dispose: () => clearTimeout(timer) },
  );
}

function deactivate() {}

module.exports = { activate, deactivate, gotoArgs };
