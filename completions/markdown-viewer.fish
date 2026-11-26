complete -c markdown-viewer -s o -l output -d 'Write the rendered HTML to this file ("-" for stdout) instead of opening it. For a folder, write a static site into this folder' -r -F
complete -c markdown-viewer -l pdf -d 'Write a PDF to this file instead of opening it (needs Chrome, Chromium, Edge, Brave or Vivaldi)' -r -F
complete -c markdown-viewer -l goto -d 'Scroll a running viewer to FILE:LINE, starting one if needed. For editor commands (see editors/ in the repository)' -r
complete -c markdown-viewer -s p -l port -d 'Port for --watch (default: any free port)' -r
complete -c markdown-viewer -l browser -d 'Browser to open instead of the default (e.g. "Google Chrome", "firefox")' -r
complete -c markdown-viewer -l theme -d 'Colour theme (default: the one you last chose in the viewer, else auto). Press t in the viewer to switch' -r -f -a "auto\t'Follow the operating system\'s light/dark setting'
light\t''
dark\t''"
complete -c markdown-viewer -l style -d 'Page style' -r -f -a "github\t'GitHub\'s README look'
wide\t'GitHub\'s look, using the full window width'
book\t'Serif type and a narrower column, for long-form reading'
compact\t'Smaller type and tighter spacing, for dense documents'"
complete -c markdown-viewer -l css -d 'Extra CSS file, applied after the built-in styles' -r -F
complete -c markdown-viewer -l front-matter -d 'YAML front matter: shown as a table, like GitHub (default), or hidden' -r -f -a "table\t'Show it as a table, like GitHub'
hide\t'Leave it out'"
complete -c markdown-viewer -l editor -d 'Editor for double-click-to-source, e.g. "zed" or "code -g {file}:{line}" (default: $VISUAL/$EDITOR if it\'s a GUI editor, else the first one found)' -r
complete -c markdown-viewer -l generate-completions -d 'Print a shell completion script (bash, zsh, fish, elvish, powershell)' -r -f -a "bash\t''
elvish\t''
fish\t''
powershell\t''
zsh\t''"
complete -c markdown-viewer -s w -l watch -d 'Serve the page and live-reload it whenever the file changes'
complete -c markdown-viewer -l clipboard -d 'Preview the clipboard instead of a file. With --watch, the page follows the clipboard as it changes'
complete -c markdown-viewer -l if-running -d 'With --goto: do nothing if no viewer is showing the file (for editors following the cursor, so a closed preview stays closed)'
complete -c markdown-viewer -l check-links -d 'Check links and images in FILE (or every Markdown file in a folder) and report broken ones; exits with status 1 if any are'
complete -c markdown-viewer -l recent -d 'Show the files and folders you opened recently (also what you get with no FILE in your home folder)'
complete -c markdown-viewer -l online -d 'With --check-links: also check web links (needs curl)'
complete -c markdown-viewer -l lint -d 'Report writing problems in FILE (or every Markdown file in a folder): skipped heading levels, images without alt text, empty links and more; exits with status 1 if there are any. Combines with --check-links'
complete -c markdown-viewer -l no-open -d 'Don\'t open a browser'
complete -c markdown-viewer -l app -d 'Open as a minimal app window with no tabs or address bar (default). Chrome, Edge, Brave, Chromium and Vivaldi only; other browsers get a new window'
complete -c markdown-viewer -l window -d 'Open in a normal new browser window'
complete -c markdown-viewer -l tab -d 'Open in a tab of an existing browser window'
complete -c markdown-viewer -l exit-when-closed -d 'With --watch: exit once the page has been closed, even with --no-open (used by the Finder integration). Without --no-open, this is the default'
complete -c markdown-viewer -l keep-running -d 'With --watch: keep serving after the window is closed (until Ctrl-C)'
complete -c markdown-viewer -l background -d 'Internal: this is the background viewer started for a page opened without --watch (see `launch_in_background`)'
complete -c markdown-viewer -l number-headings -d 'Number headings (1, 1.1, 1.2, …)'
complete -c markdown-viewer -l line-numbers -d 'Number the lines of every code block (or per block, with `showLineNumbers` in its info string)'
complete -c markdown-viewer -l obsidian -d 'Obsidian notes: [[wiki links]], ![[embeds]], callouts, ==highlights== and %%comments%%. On by default inside a vault (a .obsidian folder)'
complete -c markdown-viewer -l no-obsidian -d 'Turn off Obsidian notes, even inside a vault'
complete -c markdown-viewer -l generate-man -d 'Print the man page'
complete -c markdown-viewer -l install-finder -d 'macOS: add "Markdown Viewer" to Finder (Open With, double-click and a right-click Quick Action) for the current user'
complete -c markdown-viewer -l uninstall-finder -d 'macOS: remove the Finder integration'
complete -c markdown-viewer -l install-desktop -d 'Linux: add "Markdown Viewer" to your desktop\'s applications, for Open With on Markdown, CSV and TSV files (current user only)'
complete -c markdown-viewer -l uninstall-desktop -d 'Linux: remove the desktop integration'
complete -c markdown-viewer -s h -l help -d 'Print help (see more with \'--help\')'
complete -c markdown-viewer -s V -l version -d 'Print version'
