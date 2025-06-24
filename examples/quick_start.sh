#!/bin/bash

echo "GhostBridge Quick Start"
echo "======================"

# Check if Zig is installed
if ! command -v zig &> /dev/null; then
    echo "Error: Zig is not installed. Please install Zig first."
    exit 1
fi

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust is not installed. Please install Rust first."
    exit 1
fi

echo "Building GhostBridge server (Zig)..."
cd zig-server
zig build || { echo "Failed to build Zig server"; exit 1; }

echo ""
echo "Building GhostBridge client (Rust)..."
cd ../rust-client
cargo build --release || { echo "Failed to build Rust client"; exit 1; }

echo ""
echo "Starting GhostBridge server..."
cd ../zig-server
./zig-out/bin/ghostbridge --bind=127.0.0.1:9090 &
SERVER_PID=$!

echo "Server started with PID: $SERVER_PID"
echo "Waiting for server to initialize..."
sleep 2

echo ""
echo "Running client example..."
cd ../rust-client
cargo run --bin ghostbridge-example

echo ""
echo "Stopping server..."
kill $SERVER_PID

echo "Done!"