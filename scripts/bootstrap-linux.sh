#!/usr/bin/env bash
# lightMock - Bootstrap Linux (Debian/Ubuntu)
# Usage: chmod +x scripts/bootstrap-linux.sh && ./scripts/bootstrap-linux.sh
# Idempotent: safe to run multiple times
set -euo pipefail

step() { echo -e "\n=== $1 ==="; }
ok()   { echo "  OK: $1"; }
skip() { echo "  SKIP: $1"; }

step "1/6 - Rust toolchain"
if command -v rustc &>/dev/null; then
    ok "rustc deja installe ($(rustc --version))"
else
    echo "  Installation de Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    ok "Rust installe ($(rustc --version))"
fi
source "$HOME/.cargo/env" 2>/dev/null || true

step "2/6 - Dependances systeme (Debian/Ubuntu)"
if command -v apt-get &>/dev/null; then
    NEEDED=""
    for pkg in build-essential pkg-config libssl-dev; do
        dpkg -s "$pkg" &>/dev/null || NEEDED="$NEEDED $pkg"
    done
    if [ -n "$NEEDED" ]; then
        echo "  Installation de :$NEEDED"
        sudo apt-get update -qq && sudo apt-get install -y -qq $NEEDED
        ok "Paquets installes"
    else
        skip "Paquets deja presents"
    fi
else
    echo "  WARN: apt-get non disponible. Verifiez que build-essential, pkg-config, libssl-dev sont installes."
fi

step "3/6 - Node.js"
if command -v node &>/dev/null; then
    ok "Node.js deja installe ($(node --version))"
else
    echo "  Installation de Node.js 20 LTS..."
    if command -v curl &>/dev/null; then
        curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
        sudo apt-get install -y -qq nodejs
        ok "Node.js installe ($(node --version))"
    else
        echo "  WARN: Installez Node.js >= 20 manuellement."
    fi
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

step "4/6 - Dependances frontend"
cd "$PROJECT_DIR/frontend"
if [ -d "node_modules" ]; then
    skip "node_modules existe deja"
else
    npm install
    ok "npm install termine"
fi

step "5/6 - Build frontend"
npm run build
ok "Frontend compile dans dist/"

step "6/6 - Build backend"
cd "$PROJECT_DIR"
cargo build --release
ok "Backend compile dans target/release/"

echo ""
echo "================================================================"
echo "  lightMock pret ! Lancez avec :"
echo "  STATIC_DIR=./frontend/dist DATA_PATH=./data ./target/release/light-mock"
echo "  Puis ouvrez http://localhost:7342"
echo "================================================================"
