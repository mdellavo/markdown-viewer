# Editor integrations

Both use `markdown-viewer --goto FILE:LINE`, which scrolls a running viewer
to that line, or opens one there if none is showing the file. The preview
follows the file as you save; it shows the saved version, not unsaved edits.

## Zed

Zed's extensions can't react to the cursor moving, so this is a key binding
that shows the cursor's line in the preview (opening it the first time).

1. Copy the tasks from [`zed/tasks.json`](zed/tasks.json) into
   `~/.config/zed/tasks.json` (inside its top-level `[ … ]`).
2. Copy the binding from [`zed/keymap.json`](zed/keymap.json) into
   `~/.config/zed/keymap.json`. It uses <kbd>ctrl</kbd>-<kbd>alt</kbd>-<kbd>v</kbd>
   in Markdown files; change it if that's taken.

Then press the key in a Markdown file, or run **task: spawn** ›
**markdown-viewer: show this line**. There's also **markdown-viewer: browse
project**, which opens the project folder in the viewer.

## VS Code

A small extension with no dependencies, in [`vscode/`](vscode/):

- **markdown-viewer: Show This Line in Preview**
  (<kbd>cmd</kbd>-<kbd>alt</kbd>-<kbd>v</kbd> on macOS,
  <kbd>ctrl</kbd>-<kbd>alt</kbd>-<kbd>v</kbd> elsewhere) opens the preview at
  the cursor.
- While a preview is open, it follows the cursor. It never opens one by
  itself, so closing the preview window stops it.
  **markdown-viewer: Toggle Following the Cursor** switches this off.
- Settings: `markdownViewer.path` (if `markdown-viewer` isn't on your
  `PATH`) and `markdownViewer.followCursor`.

Install it by linking the folder into VS Code's extensions and restarting
VS Code:

```sh
ln -s "$PWD/editors/vscode" ~/.vscode/extensions/markdown-viewer
```

Its tests run without VS Code: `node editors/vscode/test/extension.test.js`.
