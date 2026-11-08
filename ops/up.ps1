# up - check first, then ship
param([Parameter(Mandatory=$true)][ValidatePattern('^[a-z0-9][a-z0-9_-]*$')][string]$Info,[string]$Msg='',[string]$Out='target')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
& "$PSScriptRoot/ck.ps1" -Out $Out
if (!$?) { throw 'ck' }
$ps = @('.gitattributes','.gitignore','Cargo.toml','Cargo.lock','app','cr','ops','.github','res')
& git add -A -- @ps
if ($LASTEXITCODE -ne 0) { throw 'add' }
$n = 0
& git log --all --format=%s | ForEach-Object { if ($_ -match '_(\d+)$') { $n = [Math]::Max($n,[int]$Matches[1]) } }
$n++
& git diff --cached --check -- @ps
if ($LASTEXITCODE -ne 0) { throw 'diff' }
$title = $Info.TrimEnd('_') + '_' + $n
if (!$Msg) { $Msg = $Info }
& git commit --only -m $title -m $Msg -- @ps
if ($LASTEXITCODE -ne 0) { throw 'commit' }
& git -c http.version=HTTP/1.1 -c http.lowSpeedLimit=1 -c http.lowSpeedTime=20 push -u origin HEAD:main
if ($LASTEXITCODE -ne 0) { throw 'push' }