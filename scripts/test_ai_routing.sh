#!/bin/bash

# Test script for AI-Routing
# This script demonstrates AI-routing by creating 3 nodes and showing how messages are routed

set -e

echo "🧹 Cleaning up old processes..."
pkill -f "target/.*/core" 2>/dev/null || true
sleep 1

echo ""
echo "🚀 Starting 3 nodes for AI-routing test..."
echo ""

# Start node 1
echo "Starting Node 1 on port 8082..."
RUST_LOG=info cargo run --bin core --release -- 8082 127.0.0.1:8083 127.0.0.1:8084 2>&1 | tee /tmp/ai_test_node1.log | grep -E "(INFO|ERROR|WARN|AI-Routing|Latency|Score)" &
N1=$!

sleep 2

# Start node 2
echo "Starting Node 2 on port 8083..."
RUST_LOG=info cargo run --bin core --release -- 8083 127.0.0.1:8082 127.0.0.1:8084 2>&1 | tee /tmp/ai_test_node2.log | grep -E "(INFO|ERROR|WARN|AI-Routing|Latency|Score)" &
N2=$!

sleep 2

# Start node 3
echo "Starting Node 3 on port 8084..."
RUST_LOG=info cargo run --bin core --release -- 8084 127.0.0.1:8082 127.0.0.1:8083 2>&1 | tee /tmp/ai_test_node3.log | grep -E "(INFO|ERROR|WARN|AI-Routing|Latency|Score)" &
N3=$!

echo ""
echo "⏳ Waiting for nodes to connect (15 seconds)..."
sleep 15

echo ""
echo "=== Node Status ==="
MESHLINK_API_PORT=17082 cargo run --bin cli -- status 2>&1 | grep -v "warning\|Compiling\|Finished" | tail -5

echo ""
echo "=== Connected Peers ==="
MESHLINK_API_PORT=17082 cargo run --bin cli -- peers 2>&1 | grep -v "warning\|Compiling\|Finished" | tail -10

echo ""
echo "⏳ Waiting for ping/pong cycles to measure latency (20 seconds)..."
sleep 20

echo ""
echo "=== Sending test message to trigger AI-routing ==="
PEER_ID=$(MESHLINK_API_PORT=17082 cargo run --bin cli -- peers 2>&1 | grep -v "warning\|Compiling\|Finished" | grep Connected | head -1 | awk '{print $1}')
if [ -n "$PEER_ID" ]; then
    echo "Sending message to peer: $PEER_ID"
    MESHLINK_API_PORT=17082 cargo run --bin cli -- send "$PEER_ID" "AI-routing test message" 2>&1 | grep -v "warning\|Compiling\|Finished"
    sleep 3
    
    echo ""
    echo "=== Checking AI-routing logs ==="
    echo "Node 1 AI-routing activity:"
    grep -E "(AI-Routing|Forwarding|Score)" /tmp/ai_test_node1.log | tail -5 || echo "No AI-routing activity yet"
    
    echo ""
    echo "Node 2 AI-routing activity:"
    grep -E "(AI-Routing|Forwarding|Score)" /tmp/ai_test_node2.log | tail -5 || echo "No AI-routing activity yet"
    
    echo ""
    echo "Node 3 AI-routing activity:"
    grep -E "(AI-Routing|Forwarding|Score)" /tmp/ai_test_node3.log | tail -5 || echo "No AI-routing activity yet"
else
    echo "⚠️ Could not find connected peer"
fi

echo ""
echo "=== Latency measurements ==="
echo "Checking for latency measurements in logs..."
grep -E "(Latency to|ping|pong)" /tmp/ai_test_node*.log | tail -10 || echo "No latency measurements yet (ping happens every 30s on timeout)"

echo ""
echo "🧹 Cleaning up..."
kill $N1 $N2 $N3 2>/dev/null || true
wait $N1 $N2 $N3 2>/dev/null || true

echo ""
echo "✅ AI-routing test completed!"
echo ""
echo "To see full logs, check:"
echo "  - /tmp/ai_test_node1.log"
echo "  - /tmp/ai_test_node2.log"
echo "  - /tmp/ai_test_node3.log"

