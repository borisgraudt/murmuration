#!/bin/bash
# Start web interface server

cd "$(dirname "$0")/../web/frontend" || exit 1

echo "🌐 Starting Elysium Web interface..."
echo "📍 Server: http://localhost:8081"
echo ""
echo "Press Ctrl+C to stop"
echo ""

python3 -m http.server 8081


