# CodeGraph CLI MCP Server - Installation Guide

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Prerequisites](#prerequisites)
3. [Installation Methods](#installation-methods)
4. [Post-Installation Setup](#post-installation-setup)
5. [Verifying Installation](#verifying-installation)
6. [Updating CodeGraph](#updating-codegraph)
7. [Uninstalling](#uninstalling)
8. [Troubleshooting Installation](#troubleshooting-installation)

## System Requirements

### Minimum Requirements

- **Operating System**: 
  - Linux (Ubuntu 20.04+, Fedora 34+, Debian 11+)
  - macOS 11.0+ (Big Sur or later)
  - Windows 10/11 with WSL2
- **Memory**: 4GB RAM
- **Storage**: 2GB free disk space
- **CPU**: Dual-core processor (x86_64 or ARM64)

### Recommended Requirements

- **Memory**: 8GB RAM or more
- **Storage**: 10GB free disk space (for large codebases)
- **CPU**: Quad-core processor or better
- **GPU**: Optional - NVIDIA GPU with CUDA support for accelerated embeddings

## Prerequisites

### 1. Install Rust

CodeGraph requires Rust 1.75 or later.

#### Linux/macOS

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Follow the on-screen instructions, then reload your shell
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### Windows

Download and run the installer from [rustup.rs](https://rustup.rs/)

### 2. Install System Dependencies

#### macOS

```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install cmake clang pkg-config openssl

# For FAISS support (optional)
brew install faiss
```

#### Ubuntu/Debian

```bash
# Update package list
sudo apt-get update

# Install build essentials
sudo apt-get install -y build-essential cmake clang pkg-config libssl-dev

# For FAISS support (optional)
sudo apt-get install -y libfaiss-dev

# For GPU support (optional)
sudo apt-get install -y nvidia-cuda-toolkit
```

#### Fedora/RHEL/CentOS

```bash
# Install development tools
sudo dnf groupinstall "Development Tools"
sudo dnf install cmake clang openssl-devel pkg-config

# For FAISS support (optional)
sudo dnf install faiss-devel
```

#### Arch Linux

```bash
# Install dependencies
sudo pacman -S base-devel cmake clang openssl pkg-config

# For FAISS from AUR (optional)
yay -S faiss
```

### 3. Install Git (Optional but Recommended)

```bash
# macOS
brew install git

# Ubuntu/Debian
sudo apt-get install git

# Fedora
sudo dnf install git

# Arch
sudo pacman -S git
```

## Installation Methods

### Method 1: Install from Source (Recommended)

This method provides the latest features and allows customization.

```bash
# Clone the repository
git clone https://github.com/your-org/codegraph-cli-mcp.git
cd codegraph-cli-mcp

# Build with optimizations
cargo build --release

# Install the binary globally
cargo install --path crates/codegraph-mcp

# Verify installation
codegraph --version
```

#### Build with Specific Features

```bash
# Build with all features
cargo build --release --all-features

# Build with specific features
cargo build --release --features "faiss,gpu"

# Build minimal version
cargo build --release --no-default-features
```

### Method 2: Install from Crates.io

When the package is published to crates.io:

```bash
# Install latest stable version
cargo install codegraph-mcp

# Install specific version
cargo install codegraph-mcp --version 1.0.0

# Install with specific features
cargo install codegraph-mcp --features "faiss"
```

### Method 3: Install Pre-built Binaries

Download pre-built binaries for your platform:

```bash
# Determine your platform
PLATFORM=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Download the appropriate binary
curl -L "https://github.com/your-org/codegraph-cli-mcp/releases/latest/download/codegraph-${PLATFORM}-${ARCH}.tar.gz" -o codegraph.tar.gz

# Extract the archive
tar -xzf codegraph.tar.gz

# Move to system path
sudo mv codegraph /usr/local/bin/

# Make executable
sudo chmod +x /usr/local/bin/codegraph

# Verify installation
codegraph --version
```

### Method 4: Docker Installation

For containerized deployments:

```bash
# Pull the official Docker image
docker pull codegraph/cli-mcp:latest

# Create an alias for easy usage
alias codegraph='docker run --rm -it -v $(pwd):/workspace codegraph/cli-mcp:latest'

# Test the installation
codegraph --version
```

#### Building Docker Image Locally

```bash
# Clone the repository
git clone https://github.com/your-org/codegraph-cli-mcp.git
cd codegraph-cli-mcp

# Build Docker image
docker build -t codegraph-local .

# Run the container
docker run --rm -it codegraph-local --version
```

### Method 5: Platform-Specific Package Managers

#### Homebrew (macOS/Linux)

```bash
# Add the tap
brew tap codegraph/tap

# Install CodeGraph
brew install codegraph

# Upgrade to latest version
brew upgrade codegraph
```

#### Snap (Linux)

```bash
# Install from Snap Store
sudo snap install codegraph

# Grant necessary permissions
sudo snap connect codegraph:home
sudo snap connect codegraph:removable-media
```

#### AUR (Arch Linux)

```bash
# Using yay
yay -S codegraph-cli-mcp

# Using paru
paru -S codegraph-cli-mcp
```

## Post-Installation Setup

### 1. Initialize Configuration

```bash
# Create default configuration directory
mkdir -p ~/.codegraph

# Initialize with default settings
codegraph init

# Or initialize with interactive setup
codegraph init --interactive
```

### 2. Set Up Environment Variables

Add to your shell configuration file (`~/.bashrc`, `~/.zshrc`, etc.):

```bash
# CodeGraph environment variables
export CODEGRAPH_HOME="$HOME/.codegraph"
export CODEGRAPH_CONFIG="$CODEGRAPH_HOME/config.toml"
export CODEGRAPH_LOG_LEVEL="info"

# Optional: API keys for embedding models
export OPENAI_API_KEY="your-api-key-here"

# Optional: Performance tuning
export CODEGRAPH_WORKERS=8
export CODEGRAPH_MEMORY_LIMIT=4096
```

### 3. Download Embedding Models (Optional)

For local embedding models:

```bash
# Create models directory
mkdir -p ~/.codegraph/models

# Download a model (example with codestral)
curl -L "https://huggingface.co/models/codestral-latest.gguf" \
     -o ~/.codegraph/models/codestral.gguf

# Configure CodeGraph to use the model
codegraph config set embedding.model local
codegraph config set embedding.local.model_path "~/.codegraph/models/codestral.gguf"
```

### 4. Configure for Your IDE

#### VS Code Integration

```bash
# Install the MCP extension
code --install-extension mcp.vscode-mcp

# Configure VS Code settings
cat >> ~/.config/Code/User/settings.json << EOF
{
  "mcp.servers": {
    "codegraph": {
      "command": "codegraph",
      "args": ["start", "stdio"]
    }
  }
}
EOF
```

#### Claude Desktop Integration

```bash
# Add to Claude configuration
cat >> ~/Library/Application\ Support/Claude/config.json << EOF
{
  "mcpServers": {
    "codegraph": {
      "command": "codegraph",
      "args": ["start", "stdio"],
      "env": {
        "CODEGRAPH_CONFIG": "~/.codegraph/config.toml"
      }
    }
  }
}
EOF
```

## Verifying Installation

### Basic Verification

```bash
# Check version
codegraph --version

# Display help
codegraph --help

# Run diagnostics
codegraph doctor
```

### Comprehensive Test

```bash
# Create a test project
mkdir -p /tmp/codegraph-test
cd /tmp/codegraph-test

# Create sample files
echo 'fn main() { println!("Hello, CodeGraph!"); }' > main.rs
echo 'def hello(): print("Hello from Python")' > hello.py

# Initialize CodeGraph
codegraph init --name test-project

# Index the files
codegraph index .

# Start the server
codegraph start stdio &
SERVER_PID=$!

# Test search
codegraph search "hello"

# Stop the server
kill $SERVER_PID

# Clean up
cd ..
rm -rf /tmp/codegraph-test
```

### System Compatibility Check

```bash
# Run system check
codegraph doctor --verbose

# Expected output:
# ✓ Rust version: 1.75.0
# ✓ System architecture: x86_64
# ✓ Available memory: 16GB
# ✓ Disk space: 50GB free
# ✓ FAISS support: Available
# ✓ GPU support: Not available
# ✓ Configuration: Valid
```

## Updating CodeGraph

### Update from Source

```bash
# Navigate to repository
cd /path/to/codegraph-cli-mcp

# Pull latest changes
git pull origin main

# Rebuild and reinstall
cargo build --release
cargo install --path crates/codegraph-mcp --force
```

### Update from Crates.io

```bash
# Update to latest version
cargo install codegraph-mcp --force

# Update to specific version
cargo install codegraph-mcp --version 1.2.0 --force
```

### Update Docker Image

```bash
# Pull latest image
docker pull codegraph/cli-mcp:latest

# Remove old image
docker image prune
```

## Uninstalling

### Remove Binary Installation

```bash
# Remove from cargo installations
cargo uninstall codegraph-mcp

# Or remove manually
sudo rm /usr/local/bin/codegraph
```

### Clean Configuration and Data

```bash
# Backup your data first (optional)
tar -czf codegraph-backup.tar.gz ~/.codegraph

# Remove configuration and data
rm -rf ~/.codegraph

# Remove environment variables from shell config
# Edit ~/.bashrc, ~/.zshrc, etc. and remove CodeGraph entries
```

### Remove Docker Installation

```bash
# Remove Docker image
docker rmi codegraph/cli-mcp:latest

# Remove all CodeGraph containers
docker rm $(docker ps -a -q --filter ancestor=codegraph/cli-mcp)
```

## Troubleshooting Installation

### Common Issues

#### Issue: "cargo: command not found"

**Solution:**
```bash
# Ensure Rust is installed and in PATH
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

#### Issue: Build fails with "linking with cc failed"

**Solution:**
```bash
# Install C compiler and linker
# Ubuntu/Debian
sudo apt-get install build-essential

# macOS
xcode-select --install

# Fedora
sudo dnf install gcc
```

#### Issue: "FAISS not found" during build

**Solution:**
```bash
# Build without FAISS support
cargo build --release --no-default-features --features "base"

# Or install FAISS
# macOS
brew install faiss

# Ubuntu
sudo apt-get install libfaiss-dev
```

#### Issue: Permission denied when installing globally

**Solution:**
```bash
# Use sudo for system-wide installation
sudo mv codegraph /usr/local/bin/

# Or install to user directory
mkdir -p ~/.local/bin
mv codegraph ~/.local/bin/
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

#### Issue: SSL/TLS certificate errors

**Solution:**
```bash
# Update certificates
# Ubuntu/Debian
sudo apt-get update && sudo apt-get install ca-certificates

# macOS
brew install ca-certificates

# Set certificate bundle path
export SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
```

### Getting Help

If you encounter issues not covered here:

1. Check the [GitHub Issues](https://github.com/your-org/codegraph-cli-mcp/issues)
2. Run diagnostics: `codegraph doctor --debug`
3. Consult the [FAQ](https://docs.codegraph.dev/faq)
4. Join our [Discord community](https://discord.gg/codegraph)
5. File a bug report with:
   - System information: `codegraph doctor --system-info`
   - Error logs: `~/.codegraph/logs/error.log`
   - Steps to reproduce the issue

---

## Next Steps

After successful installation:

1. [Quick Start Guide](./QUICK_START.md) - Get up and running quickly
2. [Configuration Guide](./CONFIGURATION.md) - Customize CodeGraph for your needs
3. [CLI Reference](./CLI_REFERENCE.md) - Complete command documentation
4. [Integration Guide](./INTEGRATION.md) - Connect with your development tools