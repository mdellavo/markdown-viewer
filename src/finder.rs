//! macOS Finder integration: `--install-finder` / `--uninstall-finder`.
//!
//! Installs, for the current user only (no admin rights needed):
//!
//! - `~/Applications/Markdown Viewer.app`: a small AppleScript app that
//!   registers for Markdown files, so they can be opened with Open With,
//!   double-click (once set as default), or by dropping them on its icon.
//!   Each file is opened with `markdown-viewer --watch --exit-when-closed`.
//! - `~/Library/Services/Open in Markdown Viewer.workflow`: a Quick Action
//!   in Finder's right-click menu that hands the selection to the app.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const APP_NAME: &str = "Markdown Viewer";
const BUNDLE_ID: &str = "io.github.mdellavo.markdown-viewer";
const WORKFLOW_NAME: &str = "Open in Markdown Viewer";
const ICON: &[u8] = include_bytes!("../assets/macos/AppIcon.icns");
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

/// File extensions we declare as Markdown, in case the system doesn't know
/// some of them.
const EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mkd", "mkdn"];

struct Paths {
    app: PathBuf,
    workflow: PathBuf,
    log: PathBuf,
}

fn paths() -> Result<Paths> {
    let home = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?);
    Ok(Paths {
        app: home.join("Applications").join(format!("{APP_NAME}.app")),
        workflow: home
            .join("Library/Services")
            .join(format!("{WORKFLOW_NAME}.workflow")),
        log: home.join("Library/Logs/markdown-viewer.log"),
    })
}

pub fn install() -> Result<()> {
    let p = paths()?;
    let exe = stable_exe_path()?;

    let staging = tempfile::tempdir()?;
    let built = staging.path().join(format!("{APP_NAME}.app"));
    build_app(&built, &exe, &p.log)?;
    replace_dir(&built, &p.app)?;
    run(Command::new(LSREGISTER).arg("-f").arg(&p.app))?;

    write_workflow(&p.workflow)?;
    // Make the Quick Action show up without logging out.
    let _ = Command::new("/System/Library/CoreServices/pbs")
        .arg("-update")
        .status();

    println!("Installed {}", p.app.display());
    println!("Installed {}", p.workflow.display());
    println!();
    println!("In Finder you can now:");
    println!("  - right-click a Markdown file > Open With > {APP_NAME}");
    println!("  - right-click a Markdown file > Quick Actions > {WORKFLOW_NAME}");
    println!("  - make it the default: Get Info on a .md file > Open with >");
    println!("    {APP_NAME} > Change All…");
    println!();
    println!("The app runs {}.", exe.display());
    println!("If that moves, run `markdown-viewer --install-finder` again.");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let p = paths()?;
    let mut removed = false;
    if p.app.exists() {
        let _ = Command::new(LSREGISTER).arg("-u").arg(&p.app).status();
        std::fs::remove_dir_all(&p.app)
            .with_context(|| format!("cannot remove {}", p.app.display()))?;
        println!("Removed {}", p.app.display());
        removed = true;
    }
    if p.workflow.exists() {
        std::fs::remove_dir_all(&p.workflow)
            .with_context(|| format!("cannot remove {}", p.workflow.display()))?;
        let _ = Command::new("/System/Library/CoreServices/pbs")
            .arg("-update")
            .status();
        println!("Removed {}", p.workflow.display());
        removed = true;
    }
    if !removed {
        println!("Finder integration is not installed.");
    }
    Ok(())
}

/// The path the app should run. Homebrew installs into a versioned Cellar
/// directory that changes on every upgrade, so prefer its stable
/// `<prefix>/bin` link when we're running from one.
fn stable_exe_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot find this program's path")?;
    let parts: Vec<_> = exe.components().collect();
    if let Some(i) = parts.iter().position(|c| c.as_os_str() == "Cellar") {
        let prefix: PathBuf = parts[..i].iter().collect();
        let linked = prefix.join("bin").join("markdown-viewer");
        if linked.exists() {
            return Ok(linked);
        }
    }
    Ok(exe)
}

fn build_app(app: &Path, exe: &Path, log: &Path) -> Result<()> {
    let script = app.with_extension("applescript");
    std::fs::write(&script, applescript(exe, log))?;
    run(Command::new("osacompile").arg("-o").arg(app).arg(&script))?;

    let res = app.join("Contents/Resources");
    // Swap the generic droplet icon for ours.
    let _ = std::fs::remove_file(res.join("Assets.car"));
    let _ = std::fs::remove_file(res.join("droplet.icns"));
    std::fs::write(res.join("AppIcon.icns"), ICON)?;

    let plist = app.join("Contents/Info.plist");
    let set = |args: &[&str]| run(Command::new("plutil").args(args).arg(&plist));
    let _ = set(&["-remove", "CFBundleIconName"]);
    set(&["-replace", "CFBundleIconFile", "-string", "AppIcon"])?;
    set(&["-replace", "CFBundleIdentifier", "-string", BUNDLE_ID])?;
    set(&["-replace", "CFBundleName", "-string", APP_NAME])?;
    set(&["-replace", "CFBundleDisplayName", "-string", APP_NAME])?;
    set(&[
        "-replace",
        "CFBundleShortVersionString",
        "-string",
        env!("CARGO_PKG_VERSION"),
    ])?;
    set(&[
        "-replace",
        "CFBundleDocumentTypes",
        "-json",
        &document_types(),
    ])?;
    set(&[
        "-replace",
        "UTImportedTypeDeclarations",
        "-json",
        &imported_types(),
    ])?;

    // Editing the bundle invalidated osacompile's ad-hoc signature.
    run(Command::new("codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(app))?;
    Ok(())
}

/// The app's script. Each file is opened in its own background viewer that
/// exits when its window is closed; output goes to a log for troubleshooting.
fn applescript(exe: &Path, log: &Path) -> String {
    format!(
        r#"-- Generated by `markdown-viewer --install-finder`.
property mdvPath : {exe}
property logPath : {log}

on open theItems
	repeat with anItem in theItems
		my viewFile(POSIX path of anItem)
	end repeat
end open

on run
	set chosen to choose file with prompt "Choose a Markdown file to view:" of type {{"net.daringfireball.markdown", "public.plain-text"}} with multiple selections allowed
	repeat with anItem in chosen
		my viewFile(POSIX path of anItem)
	end repeat
end run

on viewFile(p)
	do shell script quoted form of mdvPath & " --watch --exit-when-closed " & quoted form of p & " >> " & quoted form of logPath & " 2>&1 &"
end viewFile
"#,
        exe = applescript_string(&exe.to_string_lossy()),
        log = applescript_string(&log.to_string_lossy()),
    )
}

fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn document_types() -> String {
    format!(
        r#"[{{"CFBundleTypeName":"Markdown document","CFBundleTypeRole":"Viewer","LSHandlerRank":"Alternate","LSItemContentTypes":["net.daringfireball.markdown"],"CFBundleTypeExtensions":{exts}}}]"#,
        exts = json_list(EXTENSIONS)
    )
}

fn imported_types() -> String {
    format!(
        r#"[{{"UTTypeIdentifier":"net.daringfireball.markdown","UTTypeDescription":"Markdown document","UTTypeConformsTo":["public.plain-text"],"UTTypeReferenceURL":"https://daringfireball.net/projects/markdown/","UTTypeTagSpecification":{{"public.filename-extension":{exts},"public.mime-type":["text/markdown"]}}}}]"#,
        exts = json_list(EXTENSIONS)
    )
}

fn json_list(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| format!("\"{s}\"")).collect();
    format!("[{}]", quoted.join(","))
}

/// The Quick Action: a Finder service that passes the selected files to the
/// app, so all launching logic lives in one place.
fn write_workflow(dir: &Path) -> Result<()> {
    let contents = dir.join("Contents");
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(&contents)?;
    std::fs::write(contents.join("Info.plist"), workflow_info_plist())?;
    std::fs::write(contents.join("document.wflow"), workflow_document())?;
    Ok(())
}

fn workflow_info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSBackgroundColorName</key>
			<string>background</string>
			<key>NSIconName</key>
			<string>NSActionTemplate</string>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{WORKFLOW_NAME}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
				<string>net.daringfireball.markdown</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#
    )
}

fn workflow_document() -> String {
    // `open -a` passes the files to the app as an "open documents" event.
    let command = format!("open -a \"{APP_NAME}\" \"$@\"");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>528</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.path</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{command}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/bash</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>6B6F5A0E-3C1D-4B57-9F7E-0A1B2C3D4E51</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
				</array>
				<key>OutputUUID</key>
				<string>6B6F5A0E-3C1D-4B57-9F7E-0A1B2C3D4E52</string>
				<key>UUID</key>
				<string>6B6F5A0E-3C1D-4B57-9F7E-0A1B2C3D4E53</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict/>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:253.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleIDsByPath</key>
		<dict/>
		<key>applicationPaths</key>
		<array/>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#
    )
}

/// Move `from` to `to`, replacing whatever is there.
fn replace_dir(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if to.exists() {
        std::fs::remove_dir_all(to).with_context(|| format!("cannot replace {}", to.display()))?;
    }
    // A rename fails across volumes (temp dir vs. home), so copy then.
    if std::fs::rename(from, to).is_err() {
        run(Command::new("ditto").arg(from).arg(to))?;
    }
    Ok(())
}

fn run(cmd: &mut Command) -> Result<()> {
    let out = cmd
        .output()
        .with_context(|| format!("cannot run {:?}", cmd.get_program()))?;
    if !out.status.success() {
        bail!(
            "{:?} failed: {}",
            cmd.get_program(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applescript_quotes_paths() {
        assert_eq!(applescript_string(r#"/a "b"\c"#), r#""/a \"b\"\\c""#);
        let script = applescript(
            Path::new("/opt/my bin/markdown-viewer"),
            Path::new("/l.log"),
        );
        assert!(script.contains(r#"property mdvPath : "/opt/my bin/markdown-viewer""#));
        assert!(script.contains("--watch --exit-when-closed"));
    }

    #[test]
    fn plist_fragments_are_valid_json() {
        for json in [document_types(), imported_types()] {
            serde_json::from_str::<serde_json::Value>(&json).unwrap();
        }
    }

    #[test]
    fn workflow_plists_parse() {
        let dir = tempfile::tempdir().unwrap();
        write_workflow(&dir.path().join("x.workflow")).unwrap();
        for f in ["Info.plist", "document.wflow"] {
            let path = dir.path().join("x.workflow/Contents").join(f);
            let ok = Command::new("plutil")
                .arg("-lint")
                .arg(&path)
                .status()
                .unwrap();
            assert!(ok.success(), "{f} is not a valid plist");
        }
    }
}
