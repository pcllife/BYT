# UP
param([string]$Msg = 'split ws; ui/bg/os/fs/db do their own thing')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
& "$PSScriptRoot/ck.ps1"
if (!$?) { throw 'ck' }
$ps = @('.gitattributes','.gitignore','Cargo.toml','Cargo.lock','app','cr','ops','.github','res')
& git add -A -- @ps
if ($LASTEXITCODE -ne 0) { throw 'add' }
$n = 0
& git log --all --format=%s | ForEach-Object { if ($_ -match '^info_(\d+)$') { $n = [Math]::Max($n,[int]$Matches[1]) } }
$n++
& git diff --cached --check -- @ps
if ($LASTEXITCODE -ne 0) { throw 'diff' }
& git commit --only -m "info_$n" -m $Msg -- @ps
if ($LASTEXITCODE -ne 0) { throw 'commit' }
& git -c http.version=HTTP/1.1 -c http.lowSpeedLimit=1 -c http.lowSpeedTime=20 push -u origin HEAD:main
if ($LASTEXITCODE -ne 0) { throw 'push' }