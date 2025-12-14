#!/bin/bash
# MeshLink MVP Demo Script
# Launches 3 nodes and demonstrates AI-routing + PQC encryption

set -e

echo "🚀 Starting MeshLink MVP Demo..."
echo ""

# Cleanup function
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    pkill -f "cargo run --bin core" || true
    sleep 1
}

trap cleanup EXIT

# Start node 1
echo "📡 Starting Node 1 (port 8082)..."
cargo run --bin core --release -- 8082 > /tmp/node1.log 2>&1 &
NODE1_PID=$!
sleep 2

# Start node 2
echo "📡 Starting Node 2 (port 8083)..."
cargo run --bin core --release -- 8083 127.0.0.1:8082 > /tmp/node2.log 2>&1 &
NODE2_PID=$!
sleep 2

# Start node 3
echo "📡 Starting Node 3 (port 8084)..."
cargo run --bin core --release -- 8084 127.0.0.1:8082 > /tmp/node3.log 2>&1 &
NODE3_PID=$!
sleep 3

echo "✅ All nodes started!"
echo ""
echo "📊 Checking node status..."
sleep 1

# Check status via CLI
echo ""
echo "Node 1 status:"
MESHLINK_API_PORT=17082 cargo run --bin cli -- status || true

echo ""
echo "Node 2 status:"
MESHLINK_API_PORT=17083 cargo run --bin cli -- status || true

echo ""
echo "Node 3 status:"
MESHLINK_API_PORT=17084 cargo run --bin cli -- status || true

echo ""
echo "📤 Sending broadcast message..."
MESHLINK_API_PORT=17082 cargo run --bin cli -- broadcast "MeshNet AI+PQC demo message" || true

echo ""
echo "⏳ Waiting 5 seconds for message propagation..."
sleep 5

echo ""
echo "📋 Checking peers on each node..."
echo ""
echo "Node 1 peers:"
MESHLINK_API_PORT=17082 cargo run --bin cli -- peers || true

echo ""
echo "Node 2 peers:"
MESHLINK_API_PORT=17083 cargo run --bin cli -- peers || true

echo ""
echo "Node 3 peers:"
MESHLINK_API_PORT=17084 cargo run --bin cli -- peers || true

echo ""
echo "✅ Demo complete!"
echo ""
echo "📝 Logs are available in:"
echo "  - /tmp/node1.log"
echo "  - /tmp/node2.log"
echo "  - /tmp/node3.log"
echo ""
echo "🔍 Check AI-routing logs:"
echo "  cat logs/ai_routing_logs.jsonl"
echo ""
echo "Press Ctrl+C to stop all nodes..."

# Wait for user interrupt
wait


