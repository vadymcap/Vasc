$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Find-VsDevCmd {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"

    if (Test-Path $vswhere) {
        $installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath

        if ($installPath) {
            $devCmd = Join-Path $installPath "Common7\Tools\VsDevCmd.bat"
            if (Test-Path $devCmd) {
                return $devCmd
            }
        }
    }

    $fallbacks = @(
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat"
    )

    foreach ($candidate in $fallbacks) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    return $null
}

$devCmd = Find-VsDevCmd

if (-not $devCmd) {
    Write-Host "Visual C++ Build Tools were not found." -ForegroundColor Red
    Write-Host "Install prerequisites and rerun:" -ForegroundColor Yellow
    Write-Host "winget install --id Microsoft.VisualStudio.2022.BuildTools --accept-package-agreements --accept-source-agreements --override \"--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended\""
    exit 1
}

Write-Host "Using: $devCmd" -ForegroundColor Cyan

cmd.exe /c "`"$devCmd`" -host_arch=x64 -arch=x64 > nul && cargo build --release"

if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$exePath = Join-Path $root "target\release\vasc.exe"
if (-not (Test-Path $exePath)) {
    Write-Host "Build completed but vasc.exe was not found at $exePath" -ForegroundColor Red
    exit 1
}

Write-Host "Build successful: $exePath" -ForegroundColor Green
