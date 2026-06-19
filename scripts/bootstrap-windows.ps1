# lightMock - Bootstrap Windows (PowerShell)
# Usage: .\scripts\bootstrap-windows.ps1
# Idempotent: safe to run multiple times

$ErrorActionPreference = "Stop"

function Write-Step($msg) { Write-Host "`n=== $msg ===" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "  OK: $msg" -ForegroundColor Green }
function Write-Skip($msg)  { Write-Host "  SKIP: $msg" -ForegroundColor Yellow }

Write-Step "1/6 - Rust toolchain"
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    $v = (rustc --version)
    Write-Ok "rustc deja installe ($v)"
} else {
    Write-Host "  Installation de Rust via rustup..."
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe" -UseBasicParsing
    & "$env:TEMP\rustup-init.exe" -y --default-toolchain stable
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    Write-Ok "Rust installe ($(rustc --version))"
}
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

Write-Step "2/6 - VS Build Tools (linker MSVC)"
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasVS = $false
if (Test-Path $vsWhere) {
    $vsPath = & $vsWhere -all -products * -property installationPath 2>$null
    if ($vsPath) { $hasVS = $true }
}
if ($hasVS) {
    Write-Ok "VS Build Tools deja installe"
} else {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Host "  Installation de VS Build Tools via winget..."
        winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" --accept-source-agreements --accept-package-agreements
        Write-Ok "VS Build Tools installe"
    } else {
        Write-Host "  WARN: winget non disponible. Installez manuellement VS Build Tools avec le workload C++." -ForegroundColor Red
    }
}

Write-Step "3/6 - Node.js"
if (Get-Command node -ErrorAction SilentlyContinue) {
    Write-Ok "Node.js deja installe ($(node --version))"
} else {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        winget install OpenJS.NodeJS.LTS --accept-source-agreements --accept-package-agreements
        Write-Ok "Node.js installe"
    } else {
        Write-Host "  WARN: Installez Node.js >= 20 manuellement." -ForegroundColor Red
    }
}

Write-Step "4/6 - Dependances frontend"
Set-Location "$PSScriptRoot\..\frontend"
if (Test-Path "node_modules") {
    Write-Skip "node_modules existe deja"
} else {
    npm install
    Write-Ok "npm install termine"
}

Write-Step "5/6 - Build frontend"
npm run build
Write-Ok "Frontend compile dans dist/"

Write-Step "6/6 - Build backend"
Set-Location "$PSScriptRoot\.."
# Contournement : sur certains environnements Windows, la verification de
# revocation SSL (CRL) echoue lors du telechargement des crates.
# Cette variable desactive uniquement la verification CRL cote Cargo.
$env:CARGO_HTTP_CHECK_REVOKE = "false"
$vcvars = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
if (Test-Path $vcvars) {
    cmd /c "`"$vcvars`" x64 >nul 2>&1 && cargo build --release 2>&1"
} else {
    cargo build --release
}
Write-Ok "Backend compile dans target/release/"

Write-Host "`n" -NoNewline
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  lightMock pret ! Lancez avec :" -ForegroundColor Green
Write-Host '  $env:STATIC_DIR = "frontend/dist"' -ForegroundColor White
Write-Host '  $env:DATA_PATH = "data"' -ForegroundColor White
Write-Host '  .\target\release\light-mock.exe' -ForegroundColor White
Write-Host "  Puis ouvrez http://localhost:7342" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
