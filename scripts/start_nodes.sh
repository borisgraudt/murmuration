#!/bin/bash
# Start multiple MeshLink nodes for testing

echo "🚀 Starting MeshLink nodes..."
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if nodes are already running
check_port() {
    lsof -ti:$1 > /dev/null 2>&1
}

# Start node function
start_node() {
    local port=$1
    local bootstrap=$2
    local api_port=$((9000 + port))
    
    if check_port $port; then
        echo -e "${YELLOW}⚠ Port $port is already in use${NC}"
        return 1
    fi
    
    echo -e "${GREEN}Starting node on port $port (API: $api_port)${NC}"
    
    cd "$(dirname "$0")/../core" || exit 1
    
    if [ -n "$bootstrap" ]; then
        MESHLINK_API_PORT=$api_port cargo run --bin core --release -- $port $bootstrap &
    else
        MESHLINK_API_PORT=$api_port cargo run --bin core --release -- $port &
    fi
    
    sleep 2
    echo ""
}

# Start first node (bootstrap)
echo "📡 Starting bootstrap node..."
start_node 8080

# Start second node (connects to first)
echo "📡 Starting second node..."
start_node 8081 "127.0.0.1:8080"

# Start third node (optional)
read -p "Start third node? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "📡 Starting third node..."
    start_node 8082 "127.0.0.1:8080"
fi

echo ""
echo "✅ Nodes started!"
echo ""
echo "To interact with nodes:"
echo "  Node 1: MESHLINK_API_PORT=17080 python3 python_cli/cli.py status"
echo "  Node 2: MESHLINK_API_PORT=17081 python3 python_cli/cli.py status"
echo ""
echo "Press Ctrl+C to stop all nodes"

# Wait for Ctrl+C
trap 'echo ""; echo "Stopping nodes..."; killall core 2>/dev/null; exit' INT
wait

