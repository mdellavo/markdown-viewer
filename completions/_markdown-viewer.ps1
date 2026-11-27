
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'markdown-viewer' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'markdown-viewer'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'markdown-viewer' {
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Write the rendered HTML to this file ("-" for stdout) instead of opening it. For a folder, write a static site into this folder')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Write the rendered HTML to this file ("-" for stdout) instead of opening it. For a folder, write a static site into this folder')
            [CompletionResult]::new('--pdf', '--pdf', [CompletionResultType]::ParameterName, 'Write a PDF to this file instead of opening it (needs Chrome, Chromium, Edge, Brave or Vivaldi)')
            [CompletionResult]::new('--goto', '--goto', [CompletionResultType]::ParameterName, 'Scroll a running viewer to FILE:LINE, starting one if needed. For editor commands (see editors/ in the repository)')
            [CompletionResult]::new('--diff', '--diff', [CompletionResultType]::ParameterName, 'Show FILE with what changed since a git revision marked: added text in green, removed text struck through in red (e.g. --diff HEAD for uncommitted changes, --diff HEAD~3, --diff main)')
            [CompletionResult]::new('-p', '-p', [CompletionResultType]::ParameterName, 'Port for --watch (default: any free port)')
            [CompletionResult]::new('--port', '--port', [CompletionResultType]::ParameterName, 'Port for --watch (default: any free port)')
            [CompletionResult]::new('--browser', '--browser', [CompletionResultType]::ParameterName, 'Browser to open instead of the default (e.g. "Google Chrome", "firefox")')
            [CompletionResult]::new('--theme', '--theme', [CompletionResultType]::ParameterName, 'Colour theme (default: the one you last chose in the viewer, else auto). Press t in the viewer to switch')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'Page style')
            [CompletionResult]::new('--css', '--css', [CompletionResultType]::ParameterName, 'Extra CSS file, applied after the built-in styles')
            [CompletionResult]::new('--front-matter', '--front-matter', [CompletionResultType]::ParameterName, 'YAML front matter: shown as a table, like GitHub (default), or hidden')
            [CompletionResult]::new('--editor', '--editor', [CompletionResultType]::ParameterName, 'Editor for double-click-to-source, e.g. "zed" or "code -g {file}:{line}" (default: $VISUAL/$EDITOR if it''s a GUI editor, else the first one found)')
            [CompletionResult]::new('--generate-completions', '--generate-completions', [CompletionResultType]::ParameterName, 'Print a shell completion script (bash, zsh, fish, elvish, powershell)')
            [CompletionResult]::new('-w', '-w', [CompletionResultType]::ParameterName, 'Serve the page and live-reload it whenever the file changes')
            [CompletionResult]::new('--watch', '--watch', [CompletionResultType]::ParameterName, 'Serve the page and live-reload it whenever the file changes')
            [CompletionResult]::new('--clipboard', '--clipboard', [CompletionResultType]::ParameterName, 'Preview the clipboard instead of a file. With --watch, the page follows the clipboard as it changes')
            [CompletionResult]::new('--if-running', '--if-running', [CompletionResultType]::ParameterName, 'With --goto: do nothing if no viewer is showing the file (for editors following the cursor, so a closed preview stays closed)')
            [CompletionResult]::new('--check-links', '--check-links', [CompletionResultType]::ParameterName, 'Check links and images in FILE (or every Markdown file in a folder) and report broken ones; exits with status 1 if any are')
            [CompletionResult]::new('--recent', '--recent', [CompletionResultType]::ParameterName, 'Show the files and folders you opened recently (also what you get with no FILE in your home folder)')
            [CompletionResult]::new('--online', '--online', [CompletionResultType]::ParameterName, 'With --check-links: also check web links (needs curl)')
            [CompletionResult]::new('--lint', '--lint', [CompletionResultType]::ParameterName, 'Report writing problems in FILE (or every Markdown file in a folder): skipped heading levels, images without alt text, empty links and more; exits with status 1 if there are any. Combines with --check-links')
            [CompletionResult]::new('--no-open', '--no-open', [CompletionResultType]::ParameterName, 'Don''t open a browser')
            [CompletionResult]::new('--app', '--app', [CompletionResultType]::ParameterName, 'Open as a minimal app window with no tabs or address bar (default). Chrome, Edge, Brave, Chromium and Vivaldi only; other browsers get a new window')
            [CompletionResult]::new('--window', '--window', [CompletionResultType]::ParameterName, 'Open in a normal new browser window')
            [CompletionResult]::new('--tab', '--tab', [CompletionResultType]::ParameterName, 'Open in a tab of an existing browser window')
            [CompletionResult]::new('--exit-when-closed', '--exit-when-closed', [CompletionResultType]::ParameterName, 'With --watch: exit once the page has been closed, even with --no-open (used by the Finder integration). Without --no-open, this is the default')
            [CompletionResult]::new('--keep-running', '--keep-running', [CompletionResultType]::ParameterName, 'With --watch: keep serving after the window is closed (until Ctrl-C)')
            [CompletionResult]::new('--background', '--background', [CompletionResultType]::ParameterName, 'Internal: this is the background viewer started for a page opened without --watch (see `launch_in_background`)')
            [CompletionResult]::new('--number-headings', '--number-headings', [CompletionResultType]::ParameterName, 'Number headings (1, 1.1, 1.2, …)')
            [CompletionResult]::new('--line-numbers', '--line-numbers', [CompletionResultType]::ParameterName, 'Number the lines of every code block (or per block, with `showLineNumbers` in its info string)')
            [CompletionResult]::new('--obsidian', '--obsidian', [CompletionResultType]::ParameterName, 'Obsidian notes: [[wiki links]], ![[embeds]], callouts, ==highlights== and %%comments%%. On by default inside a vault (a .obsidian folder)')
            [CompletionResult]::new('--no-obsidian', '--no-obsidian', [CompletionResultType]::ParameterName, 'Turn off Obsidian notes, even inside a vault')
            [CompletionResult]::new('--generate-man', '--generate-man', [CompletionResultType]::ParameterName, 'Print the man page')
            [CompletionResult]::new('--install-finder', '--install-finder', [CompletionResultType]::ParameterName, 'macOS: add "Markdown Viewer" to Finder (Open With, double-click and a right-click Quick Action) for the current user')
            [CompletionResult]::new('--uninstall-finder', '--uninstall-finder', [CompletionResultType]::ParameterName, 'macOS: remove the Finder integration')
            [CompletionResult]::new('--install-desktop', '--install-desktop', [CompletionResultType]::ParameterName, 'Linux: add "Markdown Viewer" to your desktop''s applications, for Open With on Markdown, CSV and TSV files (current user only)')
            [CompletionResult]::new('--uninstall-desktop', '--uninstall-desktop', [CompletionResultType]::ParameterName, 'Linux: remove the desktop integration')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
