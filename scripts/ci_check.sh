#!/bin/bash
# Script to run the same checks as CI/CD locally

set -e

echo " Running CI/CD checks locally..."
echo ""

cd "$(dirname "$0")/../core" || exit 1

echo "1. Checking formatting..."
cargo fmt --all -- --check
echo " Formatting OK"
echo ""

echo "2. Running clippy..."
cargo clippy --all-targets -- -D warnings
echo " Clippy OK"
echo ""

echo "3. Building release binaries..."
cargo build --release --bin core --bin viz --bin cli --bin mur
echo " Build OK"
echo ""

echo "4. Checking binaries exist..."
if [ -f target/release/core ] || [ -f ../target/release/core ]; then
    echo " core binary found"
else
    echo " core binary not found"
    exit 1
fi

if [ -f target/release/viz ] || [ -f ../target/release/viz ]; then
    echo " viz binary found"
else
    echo " viz binary not found"
    exit 1
fi

if [ -f target/release/cli ] || [ -f ../target/release/cli ]; then
    echo " cli binary found"
else
    echo " cli binary not found"
    exit 1
fi

if [ -f target/release/mur ] || [ -f ../target/release/mur ]; then
    echo " mur binary found"
else
    echo " mur binary not found"
    exit 1
fi
echo ""

echo "5. Running library tests..."
cargo test --lib --verbose
echo " Library tests OK"
echo ""

echo "6. Running integration tests..."
cargo test --test test_stability --test test_multi_node --test test_routing_adaptive
echo " Integration tests OK"
echo ""

echo " All CI/CD checks passed!"
