param()

if ($args.Count -ne 1) {
    [Console]::Error.WriteLine("usage: $PSCommandPath C:\\path\\to\\watch-root\\release_notes\\target.toml")
    exit 2
}

function Convert-ToTomlString([string] $value) {
    '"' + $value.Replace('\', '\\').Replace('"', '\"') + '"'
}

$destination = $args[0]
$destinationDir = Split-Path -Parent $destination
$exampleDir = Split-Path -Parent $PSCommandPath
$releaseNotesPath = Join-Path $exampleDir 'release-notes.html'
$hookScriptPath = Join-Path $exampleDir 'append-notification.ps1'
$hookProgramPath = (Get-Process -Id $PID).Path
$hookLogPath = Join-Path $destinationDir 'ffhn-release-notes-report.jsonl'

New-Item -ItemType Directory -Force $destinationDir | Out-Null

$content = @"
schema_name = "ffhn.target"
schema_version = 1
target_id = "release_notes"
display_name = "Local Release Notes Example"
enabled = true

[target]
kind = "file"
file_path = $(Convert-ToTomlString $releaseNotesPath)

[fetch]
engine = "file"
max_bytes = 2000000

[selection]
kind = "css_selector"
selector = "main"
match = "single"
output = "outer_html"
whitespace = "normalize"
rewrite_urls = false

[compare]
basis = "canonical_text_sha256"

[[compare.canonicalization]]
kind = "trim"

[[compare.canonicalization]]
kind = "collapse_whitespace"

[storage]
history_limit = 8

[[notifications]]
name = "log-json"
on = ["changed", "failed_transient", "failed_permanent"]
program = $(Convert-ToTomlString $hookProgramPath)
args = ["-NoLogo", "-NoProfile", "-File", $(Convert-ToTomlString $hookScriptPath), $(Convert-ToTomlString $hookLogPath)]
timeout_ms = 1000
"@

Set-Content -Path $destination -Value $content -Encoding UTF8
