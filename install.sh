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
echo "  2. Run 'lit init' in a directory to create a repository"
echo "  3. See QUICKSTART.md for usage examples"
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
