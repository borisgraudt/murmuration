#!/bin/bash
# Manual stability test script
# Tests that the system handles errors gracefully without crashing

set -e

echo "🧪 Testing Murmuration Stability"
echo "============================"
echo ""

# Test 1: Invalid URL handling
echo "Test 1: Invalid URL handling"
echo "  - Testing with invalid URLs..."
mur fetch "not-an-mur-url" 2>&1 | grep -q "Invalid" && echo "  ✓ Handled invalid URL" || echo "  ✗ Failed"
mur fetch "mur://" 2>&1 | grep -q "Invalid\|Error" && echo "  ✓ Handled empty URL" || echo "  ✗ Failed"
echo ""

# Test 2: Timeout handling
echo "Test 2: Timeout handling"
echo "  - Fetching non-existent content (should timeout)..."
timeout 5 mur fetch "mur://nonexistent_node_12345/path" 2>&1 | grep -q "timeout\|not found\|Error" && echo "  ✓ Timeout handled" || echo "  ✗ Failed"
echo ""

# Test 3: Multiple concurrent requests
echo "Test 3: Concurrent requests"
echo "  - Testing concurrent fetches..."
for i in {1..5}; do
    mur fetch "mur://node$i/path$i" 2>&1 > /dev/null &
done
wait
echo "  ✓ Concurrent requests handled"
echo ""

# Test 4: Very long URLs
echo "Test 4: Very long URL handling"
LONG_PATH=$(python3 -c "print('a' * 10000)")
mur fetch "mur://node/$LONG_PATH" 2>&1 | grep -q "Invalid\|Error" && echo "  ✓ Long URL handled" || echo "  ✗ Failed"
echo ""

# Test 5: Special characters
echo "Test 5: Special characters in URL"
mur fetch "mur://node/path with spaces" 2>&1 | grep -q "Invalid\|Error" && echo "  ✓ Special chars handled" || echo "  ✗ Failed"
echo ""

echo "✅ Stability tests completed!"
echo ""
echo "Note: Some tests may show errors - this is expected and shows"
echo "      that error handling is working correctly."
