#!/bin/bash
# Test connection between two nodes

echo "🔍 Testing MeshLink node connection..."
echo ""

# Check if first node is running
if ! lsof -ti:8080 > /dev/null 2>&1; then
    echo "❌ First node is not running on port 8080"
    echo "   Start it with: cd core && MESHLINK_API_PORT=17080 cargo run --bin core --release -- 8080"
    exit 1
fi

echo "✅ First node is running on port 8080"

# Check if second node is running
if ! lsof -ti:8081 > /dev/null 2>&1; then
    echo "⚠️  Second node is not running on port 8081"
    echo "   Start it with: cd core && MESHLINK_API_PORT=17081 cargo run --bin core --release -- 8081 127.0.0.1:8080"
    exit 1
fi

echo "✅ Second node is running on port 8081"
echo ""

# Check peers on node 1
echo "📡 Checking peers on node 1..."
MESHLINK_API_PORT=17080 python3 python_cli/cli.py peers 2>&1 | grep -E "(Connected|peer|Error)" | head -5

echo ""
echo "📡 Checking peers on node 2..."
MESHLINK_API_PORT=17081 python3 python_cli/cli.py peers 2>&1 | grep -E "(Connected|peer|Error)" | head -5

echo ""
echo "✅ Connection test complete"






