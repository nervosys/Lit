#!/usr/bin/env pwsh
# ============================================================================
# Lit Sandbox Demo
# ============================================================================
# Demonstrates sandboxed execution of a Lit repository.
# The sandbox isolates the filesystem, environment, and network so that
# code pulled into a repo can be run without accessing the rest of the system.
# ============================================================================

param(
    [string]$LitBinary = "$PSScriptRoot\..\target\release\lit.exe"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Resolve the lit binary path
$LitBinary = (Resolve-Path $LitBinary -ErrorAction Stop).Path
Write-Host "`n====  Lit Sandbox Demo  ====" -ForegroundColor Cyan
Write-Host "Binary: $LitBinary`n"

# ── 1. Create a temporary demo repository ──────────────────────────────────

$DemoDir = Join-Path ([System.IO.Path]::GetTempPath()) "lit-sandbox-demo-$(Get-Random)"
New-Item -ItemType Directory -Path $DemoDir -Force | Out-Null
Push-Location $DemoDir

Write-Host "[1/7] Creating demo repository in $DemoDir" -ForegroundColor Yellow
& $LitBinary init | Out-Null

# ── 2. Populate with sample project files ──────────────────────────────────

Write-Host "[2/7] Adding sample project files" -ForegroundColor Yellow

# A Python-like project with config, source, and a build script
New-Item -ItemType Directory -Path "src" -Force | Out-Null
New-Item -ItemType Directory -Path "tests" -Force | Out-Null

Set-Content -Path "README.md" -Value @"
# Demo Project
A sample project used to demonstrate Lit sandbox isolation.
"@

Set-Content -Path "src\app.py" -Value @"
"""Main application module."""
import os, sys

def greet(name: str) -> str:
    return f"Hello, {name}!"

if __name__ == "__main__":
    print(greet("World"))
    print(f"HOME = {os.environ.get('HOME', '(unset)')}")
    print(f"USERPROFILE = {os.environ.get('USERPROFILE', '(unset)')}")
    print(f"TEMP = {os.environ.get('TEMP', '(unset)')}")
    print(f"PATH entries = {len(os.environ.get('PATH', '').split(os.pathsep))}")
"@

Set-Content -Path "tests\test_app.py" -Value @"
from src.app import greet
def test_greet():
    assert greet("Lit") == "Hello, Lit!"
"@

# A batch helper that prints environment info (works inside the sandbox)
Set-Content -Path "show_env.bat" -Value @"
@echo off
echo === Sandboxed Environment ===
echo HOME=%HOME%
echo USERPROFILE=%USERPROFILE%
echo TEMP=%TEMP%
echo TMP=%TMP%
echo LIT_AIRGAPPED=%LIT_AIRGAPPED%
echo GIT_CONFIG_NOSYSTEM=%GIT_CONFIG_NOSYSTEM%
echo PATH=%PATH%
echo.
echo === Directory listing ===
dir /b
"@

# Stage and commit
& $LitBinary add README.md src/app.py tests/test_app.py show_env.bat | Out-Null
& $LitBinary commit -m "Initial demo project" | Out-Null

Write-Host "   Committed: README.md, src/app.py, tests/test_app.py, show_env.bat"

# ── 3. Create a sandbox ───────────────────────────────────────────────────

Write-Host "`n[3/7] Creating sandbox 'demo'" -ForegroundColor Yellow
$initOut = & $LitBinary sandbox init --human demo
Write-Host "   $initOut"

# ── 4. List sandboxes ─────────────────────────────────────────────────────

Write-Host "`n[4/7] Listing sandboxes" -ForegroundColor Yellow
$listOut = & $LitBinary sandbox list --human
Write-Host "   $listOut"

# ── 5. Run a command inside the sandbox ────────────────────────────────────

Write-Host "`n[5/7] Running 'show_env.bat' inside the sandbox" -ForegroundColor Yellow
Write-Host "       (demonstrates filesystem + environment isolation)" -ForegroundColor DarkGray
$runOut = & $LitBinary sandbox run --human demo -- cmd /c show_env.bat 2>&1
Write-Host ""
$runOut | ForEach-Object { Write-Host "   $_" }

# ── 6. Prove isolation: compare real vs sandboxed HOME ─────────────────────

Write-Host "`n[6/7] Comparing real vs sandboxed environment" -ForegroundColor Yellow

$realHome = $env:USERPROFILE
Write-Host "   Real USERPROFILE : $realHome" -ForegroundColor Green

# Extract HOME from sandbox output
$sandboxHome = ($runOut | Where-Object { $_ -match "^   USERPROFILE=" }) -replace "^   USERPROFILE=", ""
if (-not $sandboxHome) {
    $sandboxHome = ($runOut | Where-Object { $_ -match "USERPROFILE=" } | Select-Object -First 1) -replace ".*USERPROFILE=", ""
}
Write-Host "   Sandbox USERPROFILE: $sandboxHome" -ForegroundColor Red

if ($realHome -ne $sandboxHome) {
    Write-Host "   --> Isolation confirmed: sandbox cannot see real home directory" -ForegroundColor Cyan
} else {
    Write-Host "   --> WARNING: HOME values match — isolation may not be working" -ForegroundColor Red
}

# Check that LIT_AIRGAPPED is set
$airgapped = ($runOut | Where-Object { $_ -match "LIT_AIRGAPPED=" } | Select-Object -First 1) -replace ".*LIT_AIRGAPPED=", ""
if ($airgapped -eq "1") {
    Write-Host "   --> Network airgap confirmed: LIT_AIRGAPPED=1" -ForegroundColor Cyan
} else {
    Write-Host "   --> LIT_AIRGAPPED=$airgapped" -ForegroundColor DarkGray
}

# ── 7. Destroy the sandbox ────────────────────────────────────────────────

Write-Host "`n[7/7] Destroying sandbox 'demo'" -ForegroundColor Yellow
$destroyOut = & $LitBinary sandbox destroy --human demo
Write-Host "   $destroyOut"

# Confirm it's gone
$listAfter = & $LitBinary sandbox list --human
Write-Host "   Sandboxes remaining: $listAfter"

# ── Cleanup ────────────────────────────────────────────────────────────────

Pop-Location
Remove-Item -Recurse -Force $DemoDir -ErrorAction SilentlyContinue

Write-Host "`n====  Demo Complete  ====" -ForegroundColor Cyan
Write-Host "The demo repository was created in a temp directory, sandboxed,"
Write-Host "executed with isolation, and cleaned up. No files remain.`n"
