# ck - run the lot before a push
param([string]$Out = 'target')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$Out = if ([IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path (Get-Location) $Out }
$prev = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = $Out
try {
foreach ($file in @('ck.ps1','up.ps1')) {
    $tokens = $null; $errors = $null
    [Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $file),[ref]$tokens,[ref]$errors) | Out-Null
    if ($errors.Count) { throw ('ps parse ' + $file + ': ' + ($errors -join '; ')) }
}
function c($a) {
    & cargo @a
    if ($LASTEXITCODE -ne 0) { throw "cargo $($a -join ' ')" }
}
c @('fmt','--all','--','--check')
c @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
c @('build','--workspace','--locked')
c @('test','--workspace','--locked','--','--test-threads=1')
c @('build','--release','--workspace','--locked')
& git diff --check -- .gitattributes .gitignore Cargo.toml Cargo.lock app cr res ops .github
if ($LASTEXITCODE -ne 0) { throw 'diff' }
$e = (Resolve-Path (Join-Path $Out 'release/byt.exe')).Path
if (!(Test-Path -LiteralPath (Join-Path $Out 'release/bg.exe'))) { throw 'bg exe' }
$p = Join-Path ([IO.Path]::GetTempPath()) ('byt_ck_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $p 'files') -Force | Out-Null
function xr($a) {
    $v = Start-Process -FilePath $e -ArgumentList $a -WindowStyle Hidden -PassThru
    if (!$v.WaitForExit(20000)) { $v.Kill(); throw 'timeout' }
    if ($v.ExitCode -ne 0) { throw ('exe ' + $v.ExitCode) }
}
$b = $null
try {
    xr "--data `"$p`" --add `"$e`" --dir `"$p\files`""
    $b = Start-Process -FilePath $e -ArgumentList "--data `"$p`" --bg --ticks 4" -WindowStyle Hidden -PassThru
    xr "--data `"$p`" --ui-test"
    $ui = (Get-Content -LiteralPath (Join-Path $p 'ui.txt') -Raw).Trim()
    if ($ui -notin @('ok','no-shell')) { throw ('tray ' + $ui) }
    if ($b.HasExited) { throw 'bg stopped with ui' }
    if (!$b.WaitForExit(20000)) { throw 'bg timeout' }
    if ($b.ExitCode -ne 0) { throw 'bg exit' }
    xr "--data `"$p`" --ck"
    xr "--data `"$p`" --dump `"$p\out.json`""
    $s = Get-Content -LiteralPath (Join-Path $p 'out.json') -Encoding UTF8 -Raw | ConvertFrom-Json
    if ($s.apps.Count -ne 1 -or !$s.apps[0].z.ok -or $s.apps[0].s.ms -lt 1000) { throw 'snapshot' }
    if (Test-Path -LiteralPath (Join-Path $p 'err.txt')) { throw 'err.txt' }
    Write-Host ('smoke ok; net=' + $s.net + '; gpu=' + $s.gpu + '; tray=' + $ui)
} finally {
    if ($b -and !$b.HasExited) { $b.Kill(); $b.WaitForExit() }
    $q = [IO.Path]::GetFullPath($p)
    $t = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (!$q.StartsWith($t, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $q -Leaf) -notmatch '^byt_ck_[0-9a-f]{32}$') { throw 'tmp path' }
    Remove-Item -LiteralPath $q -Recurse -Force
}
# IDX
$ok = '^(\.gitignore|\.gitattributes|Cargo\.toml|Cargo\.lock|LICENSE|app/.*(?:\.rs|/Cargo\.toml)|cr/.*(?:\.rs|\.s|\.asm|/Cargo\.toml)|ops/(?:ck|up)\.ps1|\.github/workflows/[^/]+\.ya?ml|res/(?:f\.ttf|ofl\.txt))$'
& git ls-files | ForEach-Object { if ($_ -notmatch $ok) { throw ('blocked: ' + $_) } }

} finally { $env:CARGO_TARGET_DIR = $prev }
