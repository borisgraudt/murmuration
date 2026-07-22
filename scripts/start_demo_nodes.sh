#!/bin/bash
# Скрипт для запуска демонстрационных узлов

set -euo pipefail

echo "🚀 Запуск демонстрационных узлов Murmuration"
echo ""

# Переустановить бинарник (чтобы использовать последнюю версию)
echo "0️⃣ Переустанавливаю бинарник mur (чтобы использовать последнюю версию)..."
cargo build --release --bin mur >/dev/null 2>&1
cargo install --path core --bin mur --force >/dev/null 2>&1
echo "   ✓ Бинарник обновлен"
echo ""

# Остановить все существующие узлы
echo "1️⃣ Останавливаю все существующие узлы..."
pkill -f "mur start" || true
sleep 1

# Удалить файл, который может мешать (ВАЖНО: этот файл переопределяет --port!)
echo "2️⃣ Удаляю ~/.murmuration_api_port (если существует)..."
echo "   ⚠️  Этот файл может переопределять флаг --port!"
rm -f ~/.murmuration_api_port

# Запустить узлы
echo ""
echo "3️⃣ Запускаю узлы..."
echo ""

echo "   📍 Узел 1: P2P 8080 → API ~17080 → Gateway 8000"
mur start 8080 --gateway 8000 -d
sleep 2

echo "   📍 Узел 2: P2P 8081 → API ~17081 → Gateway 8001"
mur start 8081 127.0.0.1:8080 --gateway 8001 -d
sleep 2

echo "   📍 Узел 3: P2P 8082 → API ~17082 → Gateway 8002"
mur start 8082 127.0.0.1:8081 --gateway 8002 -d
sleep 2

echo ""
echo "✅ Узлы запущены!"
echo ""

# Найти реальные API порты
echo "4️⃣ Определяю реальные API порты из логов..."
echo ""

API_PORT_1=$(grep -h "API server listening" .mur/node-8080/node-8080.log 2>/dev/null | grep -oE "127\.0\.0\.1:[0-9]+" | cut -d: -f2 || echo "17080")
API_PORT_2=$(grep -h "API server listening" .mur/node-8081/node-8081.log 2>/dev/null | grep -oE "127\.0\.0\.1:[0-9]+" | cut -d: -f2 || echo "17081")
API_PORT_3=$(grep -h "API server listening" .mur/node-8082/node-8082.log 2>/dev/null | grep -oE "127\.0\.0\.1:[0-9]+" | cut -d: -f2 || echo "17082")

echo "   Узел 1 (P2P 8080): API порт = $API_PORT_1"
echo "   Узел 2 (P2P 8081): API порт = $API_PORT_2"
echo "   Узел 3 (P2P 8082): API порт = $API_PORT_3"
echo ""

# Проверить статус каждого узла
echo "5️⃣ Проверяю статус каждого узла..."
echo ""

echo "   Узел 1:"
mur status --port "$API_PORT_1" 2>&1 | head -5 || echo "   ⚠️ Не удалось подключиться"
echo ""

echo "   Узел 2:"
mur status --port "$API_PORT_2" 2>&1 | head -5 || echo "   ⚠️ Не удалось подключиться"
echo ""

echo "   Узел 3:"
mur status --port "$API_PORT_3" 2>&1 | head -5 || echo "   ⚠️ Не удалось подключиться"
echo ""

echo "✅ Готово!"
echo ""
echo "📋 Команды для использования:"
echo ""
echo "   # Проверить статус узлов"
echo "   mur status --port $API_PORT_1  # Узел 1"
echo "   mur status --port $API_PORT_2  # Узел 2"
echo "   mur status --port $API_PORT_3  # Узел 3"
echo ""
echo "   # Показать пиры"
echo "   mur peers --port $API_PORT_1"
echo ""
echo "   # Опубликовать контент"
echo "   mur publish site/index.html \"<h1>Test</h1>\" --port $API_PORT_1"
echo ""
echo "   # Остановить все узлы"
echo "   pkill -f 'mur start'"
echo ""
