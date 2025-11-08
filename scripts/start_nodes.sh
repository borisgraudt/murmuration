#!/bin/bash

# Скрипт для запуска 10 нод MeshLink
# Использование: ./scripts/start_nodes.sh

cd "$(dirname "$0")/.." || exit 1

START_PORT=8082
NUM_NODES=10

echo "🚀 Запуск $NUM_NODES нод MeshLink..."
echo ""

# Запускаем первую ноду (без peer)
echo "Запуск ноды 1 на порту $START_PORT..."
cargo run --bin core -- $START_PORT > /tmp/meshlink_node_${START_PORT}.log 2>&1 &
NODE1_PID=$!
echo "  ✓ Нода 1 запущена (PID: $NODE1_PID, порт: $START_PORT)"
echo "  Логи: /tmp/meshlink_node_${START_PORT}.log"
sleep 2  # Даем время первой ноде запуститься

# Запускаем остальные ноды
for i in $(seq 2 $NUM_NODES); do
    PORT=$((START_PORT + i - 1))
    PREV_PORT=$((PORT - 1))
    
    echo "Запуск ноды $i на порту $PORT (подключение к 127.0.0.1:$PREV_PORT)..."
    cargo run --bin core -- $PORT 127.0.0.1:$PREV_PORT > /tmp/meshlink_node_${PORT}.log 2>&1 &
    echo "  ✓ Нода $i запущена (PID: $!, порт: $PORT)"
    echo "  Логи: /tmp/meshlink_node_${PORT}.log"
    sleep 1  # Небольшая задержка между запусками
done

echo ""
echo "✅ Все $NUM_NODES нод запущены!"
echo ""
echo "Порты:"
for i in $(seq 1 $NUM_NODES); do
    PORT=$((START_PORT + i - 1))
    API_PORT=$((9000 + PORT))
    echo "  Нода $i: порт $PORT, API порт $API_PORT"
done
echo ""
echo "Для остановки всех нод выполните:"
echo "  pkill -f 'cargo run --bin core'"
echo ""
echo "Для просмотра логов:"
echo "  tail -f /tmp/meshlink_node_*.log"

