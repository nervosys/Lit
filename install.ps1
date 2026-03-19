# lit Installation Script for Windows
# Run with: powershell -ExecutionPolicy Bypass -File install.ps1

Write-Host "================================" -ForegroundColor Cyan
Write-Host "Lit - The Agentic-First Distributed VCS" -ForegroundColor Cyan
Write-Host "Installation Script" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""

# Check for Cargo
$cargoExists = Get-Command cargo -ErrorAction SilentlyContinue

if (-not $cargoExists) {
    Write-Host "Error: Cargo not found. Please install Rust first:" -ForegroundColor Red
    Write-Host "  https://rustup.rs/" -ForegroundColor Yellow
    exit 1
}

Write-Host "✓ Cargo found" -ForegroundColor Green

# Build release
Write-Host ""
Write-Host "Building lit (release mode)..." -ForegroundColor Yellow
cargo build --release

if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ Build successful" -ForegroundColor Green
}
else {
    Write-Host "✗ Build failed" -ForegroundColor Red
    exit 1
}

# Install
Write-Host ""
Write-Host "Installing Lit..." -ForegroundColor Yellow

# Build release binary
cargo install --path .

if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ Installation successful" -ForegroundColor Green
}
else {
    Write-Host "✗ Installation failed" -ForegroundColor Red
    exit 1
}

# Setup configuration
Write-Host ""
Write-Host "Setting up configuration..." -ForegroundColor Yellow

$configPath = Join-Path $env:USERPROFILE ".litconfig"

if (Test-Path $configPath) {
    Write-Host "⚠ Configuration already exists at $configPath" -ForegroundColor Yellow
    $response = Read-Host "Overwrite? (y/N)"
    if ($response -eq 'y' -or $response -eq 'Y') {
        Copy-Item ".litconfig.example" $configPath -Force
        Write-Host "✓ Configuration file updated" -ForegroundColor Green
    }
    else {
        Write-Host "Keeping existing configuration" -ForegroundColor Yellow
    }
}
else {
    Copy-Item ".litconfig.example" $configPath
    Write-Host "✓ Configuration file created at $configPath" -ForegroundColor Green
}

# Set environment variables (user-scoped, not machine-global)
Write-Host ""
Write-Host "Configuring environment variables (user scope)..." -ForegroundColor Yellow

$envVars = @{
    "LIT_OUTPUT"                  = "json"
    "OTEL_EXPORTER_OTLP_ENDPOINT" = "https://nervosys.ai/otlp"
    "OTEL_EXPORTER_OTLP_PROTOCOL" = "http/protobuf"
    "OTEL_SERVICE_NAME"           = "lit"
}

foreach ($kv in $envVars.GetEnumerator()) {
    $existing = [Environment]::GetEnvironmentVariable($kv.Key, "User")
    if ($existing) {
        Write-Host "  ⚠ $($kv.Key) already set, keeping existing value" -ForegroundColor Yellow
    }
    else {
        [Environment]::SetEnvironmentVariable($kv.Key, $kv.Value, "User")
        Set-Item -Path "env:$($kv.Key)" -Value $kv.Value
        Write-Host "  ✓ $($kv.Key) = $($kv.Value)" -ForegroundColor Green
    }
}

# OTEL auth header (sensitive — prompt if not already set)
$existingHeaders = [Environment]::GetEnvironmentVariable("OTEL_EXPORTER_OTLP_HEADERS", "User")
if (-not $existingHeaders) {
    Write-Host ""
    Write-Host "  OpenTelemetry auth header (OTEL_EXPORTER_OTLP_HEADERS) is not set." -ForegroundColor Yellow
    Write-Host "  Format: Authorization=Bearer <token>" -ForegroundColor Gray
    $otelHeaders = Read-Host "  Enter OTEL auth header (or press Enter to skip)"
    if ($otelHeaders) {
        [Environment]::SetEnvironmentVariable("OTEL_EXPORTER_OTLP_HEADERS", $otelHeaders, "User")
        $env:OTEL_EXPORTER_OTLP_HEADERS = $otelHeaders
        Write-Host "  ✓ OTEL_EXPORTER_OTLP_HEADERS configured" -ForegroundColor Green
    }
    else {
        Write-Host "  ⚠ Skipped — set manually later with:" -ForegroundColor Yellow
        Write-Host '    [Environment]::SetEnvironmentVariable("OTEL_EXPORTER_OTLP_HEADERS", "Authorization=Bearer <token>", "User")' -ForegroundColor Gray
    }
}
else {
    Write-Host "  ⚠ OTEL_EXPORTER_OTLP_HEADERS already set, keeping existing value" -ForegroundColor Yellow
}

# Verify installation
Write-Host ""
Write-Host "Verifying installation..." -ForegroundColor Yellow

$litExists = Get-Command lit -ErrorAction SilentlyContinue

if ($litExists) {
    Write-Host "✓ lit is installed and in PATH" -ForegroundColor Green
    try {
        $version = & lit --version 2>&1
        Write-Host "  Version: $version" -ForegroundColor Gray
    }
    catch {
        Write-Host "  Version: unknown" -ForegroundColor Gray
    }
}
else {
    Write-Host "⚠ lit installed but not in PATH" -ForegroundColor Yellow
    Write-Host "  Add %USERPROFILE%\.cargo\bin to your PATH" -ForegroundColor Yellow
}

# Next steps
Write-Host ""
Write-Host "================================" -ForegroundColor Cyan
Write-Host "Installation Complete!" -ForegroundColor Green
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  1. Edit $configPath to configure your intranet networks"
Write-Host "  2. Run 'lit init' in a directory to create a repository"
Write-Host "  3. See QUICKSTART.md for usage examples"
Write-Host ""
Write-Host "Environment variables set (user scope):" -ForegroundColor Gray
Write-Host "  LIT_OUTPUT, OTEL_EXPORTER_OTLP_ENDPOINT, OTEL_EXPORTER_OTLP_PROTOCOL," -ForegroundColor Gray
Write-Host "  OTEL_SERVICE_NAME, OTEL_EXPORTER_OTLP_HEADERS" -ForegroundColor Gray
Write-Host "  Restart your shell for changes to take effect." -ForegroundColor Gray
Write-Host ""
Write-Host "Documentation:" -ForegroundColor Yellow
Write-Host "  - README.md         : Overview"
Write-Host "  - QUICKSTART.md     : Getting started"
Write-Host "  - EXAMPLES.md       : Usage examples"
Write-Host "  - ARCHITECTURE.md   : Technical details"
Write-Host "  - TESTING.md        : Testing guide"
Write-Host ""
Write-Host "Need help? Check the documentation or run 'lit --help'" -ForegroundColor Cyan
Write-Host ""
