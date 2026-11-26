# Package a Windows release build into dist/:
#   markdown-viewer-<target>.zip (markdown-viewer.exe, docs, PowerShell
#   completions) and a .sha256 checksum next to it, in the same format as
#   packaging/package.sh writes for the other platforms.
#
# Usage: pwsh packaging/package.ps1 <target>   (after cargo build --release --target <target>)
param([Parameter(Mandatory = $true)][string]$Target)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
New-Item -ItemType Directory -Force dist | Out-Null

$name = "markdown-viewer-$Target"
if (Test-Path $name) { Remove-Item -Recurse -Force $name }
New-Item -ItemType Directory $name, (Join-Path $name 'completions') | Out-Null
Copy-Item (Join-Path 'target' $Target 'release' 'markdown-viewer.exe'), README.md, LICENSE, CHANGELOG.md -Destination $name
Copy-Item (Join-Path 'completions' '_markdown-viewer.ps1') -Destination (Join-Path $name 'completions')

$zip = Join-Path 'dist' "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $name -DestinationPath $zip
Remove-Item -Recurse -Force $name

$hash = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
[IO.File]::WriteAllText((Join-Path (Get-Location) "$zip.sha256"), "$hash  $name.zip`n")
Get-ChildItem dist
