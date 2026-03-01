param(
    [Parameter(Mandatory = $true)]
    [string]$Version
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

& "$PSScriptRoot\build-windows.ps1"
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$assetName = "vasc-$Version-windows-x86_64.zip"
$releaseDir = Join-Path $root "release"
$exePath = Join-Path $root "target\release\vasc.exe"
$zipPath = Join-Path $root $assetName

New-Item -ItemType Directory -Path $releaseDir -Force | Out-Null
Copy-Item $exePath (Join-Path $releaseDir "vasc.exe") -Force
Compress-Archive -Path (Join-Path $releaseDir "vasc.exe") -DestinationPath $zipPath -Force

Write-Host "Created: $zipPath" -ForegroundColor Green
