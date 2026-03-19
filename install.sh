#!/usr/bin/env bash
# lit Installation Script

set -e

echo "================================"
echo "Lit - The Agentic-First Distributed VCS"
echo "Installation Script"
echo "================================"
echo ""

# Check for Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo "Error: Cargo not found. Please install Rust first:"
    echo "  https://rustup.rs/"
    exit 1
fi

echo "✓ Cargo found"

# Build release
echo ""
echo "Building lit (release mode)..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✓ Build successful"
else
    echo "✗ Build failed"
    exit 1
fi

# Install
echo ""
echo "Installing Lit..."
cargo install --path .

if [ $? -eq 0 ]; then
    echo "✓ Installation successful"
else
    echo "✗ Installation failed"
    exit 1
fi

# Setup configuration
echo ""
echo "Setting up configuration..."

CONFIG_PATH="$HOME/.litconfig"

if [ -f "$CONFIG_PATH" ]; then
    echo "⚠ Configuration already exists at $CONFIG_PATH"
    read -p "Overwrite? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Keeping existing configuration"
    else
        cp .litconfig.example "$CONFIG_PATH"
        echo "✓ Configuration file created"
    fi
else
    cp .litconfig.example "$CONFIG_PATH"
    echo "✓ Configuration file created at $CONFIG_PATH"
fi

# Configure environment variables (service-scoped, user-local)
echo ""
echo "Configuring environment variables (user scope)..."

OTEL_ENV_DIR="$HOME/.config/lit"
OTEL_ENV_FILE="$OTEL_ENV_DIR/env"

mkdir -p "$OTEL_ENV_DIR"

if [ -f "$OTEL_ENV_FILE" ]; then
    echo "⚠ Environment file already exists at $OTEL_ENV_FILE"
    read -p "Overwrite? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Keeping existing environment file"
        UPDATE_ENV=false
    else
        UPDATE_ENV=true
    fi
else
    UPDATE_ENV=true
fi

if [ "$UPDATE_ENV" = true ]; then
    # Prompt for OTEL auth header (sensitive — not stored in repo)
    OTEL_HEADERS=""
    echo "  OpenTelemetry auth header (OTEL_EXPORTER_OTLP_HEADERS):"
    echo "  Format: Authorization=Bearer <token>"
    read -p "  Enter OTEL auth header (or press Enter to skip): " OTEL_HEADERS

    cat > "$OTEL_ENV_FILE" << EOF
# Lit Environment Configuration
# Created by install.sh — service-scoped, user-local
# This file is sourced by your shell profile.

# Lit output format (json | human | msgpack)
export LIT_OUTPUT="json"

# OpenTelemetry Configuration
export OTEL_EXPORTER_OTLP_ENDPOINT="https://nervosys.ai/otlp"
export OTEL_EXPORTER_OTLP_PROTOCOL="http/protobuf"
export OTEL_SERVICE_NAME="lit"
EOF

    if [ -n "$OTEL_HEADERS" ]; then
        printf 'export OTEL_EXPORTER_OTLP_HEADERS="%s"\n' "$OTEL_HEADERS" >> "$OTEL_ENV_FILE"
    else
        echo '# export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer <your-token>"' >> "$OTEL_ENV_FILE"
    fi

    # Restrict permissions — file may contain bearer tokens
    chmod 600 "$OTEL_ENV_FILE"

    echo "✓ Environment file created at $OTEL_ENV_FILE (mode 600)"
fi

# Source env file from shell profiles (idempotent)
SOURCE_LINE="[ -f \"$OTEL_ENV_FILE\" ] && . \"$OTEL_ENV_FILE\""
PROFILES=()

if [ -f "$HOME/.bashrc" ]; then
    PROFILES+=("$HOME/.bashrc")
fi
if [ -f "$HOME/.zshenv" ]; then
    PROFILES+=("$HOME/.zshenv")
elif [ -f "$HOME/.zshrc" ]; then
    PROFILES+=("$HOME/.zshrc")
fi
if [ -f "$HOME/.profile" ]; then
    PROFILES+=("$HOME/.profile")
fi

# Fallback: if none exist, use .profile
if [ ${#PROFILES[@]} -eq 0 ]; then
    PROFILES=("$HOME/.profile")
fi

for profile in "${PROFILES[@]}"; do
    if ! grep -qF "$OTEL_ENV_FILE" "$profile" 2>/dev/null; then
        echo "" >> "$profile"
        echo "# Lit environment (added by install.sh)" >> "$profile"
        echo "$SOURCE_LINE" >> "$profile"
        echo "  ✓ Added source directive to $profile"
    else
        echo "  ⚠ $profile already sources $OTEL_ENV_FILE"
    fi
done

# Apply to current session
if [ -f "$OTEL_ENV_FILE" ]; then
    . "$OTEL_ENV_FILE"
fi

# Verify installation
echo ""
echo "Verifying installation..."

if command -v lit &> /dev/null; then
    echo "✓ lit is installed and in PATH"
    LIT_VERSION=$(lit --version 2>&1 || echo "unknown")
    echo "  Version: $LIT_VERSION"
else
    echo "⚠ lit installed but not in PATH"
    echo "  Add ~/.cargo/bin to your PATH"
    echo "  Example: export PATH=\"\$HOME/.cargo/bin:\$PATH\""
fi

# Next steps
echo ""
echo "================================"
echo "Installation Complete!"
echo "================================"
echo ""
echo "Next steps:"
echo "  1. Edit ~/.litconfig to configure your intranet networks"
echo "  2. Edit ~/.config/lit/env to customize environment variables"
echo "  3. Run 'lit init' in a directory to create a repository"
echo "  4. See QUICKSTART.md for usage examples"
echo ""
echo "Documentation:"
echo "  - README.md         : Overview"
echo "  - QUICKSTART.md     : Getting started"
echo "  - EXAMPLES.md       : Usage examples"
echo "  - ARCHITECTURE.md   : Technical details"
echo "  - TESTING.md        : Testing guide"
echo ""
echo "Need help? Check the documentation or run 'lit --help'"
echo ""
