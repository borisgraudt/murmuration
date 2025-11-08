# Тестирование MeshLink между устройствами

## Текущее состояние

✅ **Реализовано:**
- P2P протокол с TCP соединениями
- RSA-2048 + AES-256-GCM шифрование
- Автоматический обмен ключами при handshake
- Шифрование всех сообщений после handshake
- Peer discovery через UDP broadcast
- Flooding-based routing для mesh-сообщений
- Отказоустойчивость (retry, keepalive, heartbeat)

## Тестирование на локальной сети

### Вариант 1: Ноутбук + Телефон в одной Wi-Fi сети

1. **На ноутбуке:**
```bash
# Узнайте IP адрес ноутбука
ifconfig  # macOS/Linux
ipconfig  # Windows

# Запустите ноду на порту 8080
cargo run --bin core -- 8080
```

2. **На телефоне (если есть Rust):**
```bash
# Узнайте IP адрес телефона
# Запустите ноду на другом порту, указав IP ноутбука
cargo run --bin core -- 8081 192.168.1.100:8080
```

### Вариант 2: Через USB/ADB (Android)

Если на телефоне нет Rust, можно использовать ADB для проброса портов:

```bash
# На ноутбуке
adb reverse tcp:8081 tcp:8081
adb shell "cargo run --bin core -- 8081 127.0.0.1:8080"
```

## Кросс-платформенная компиляция

### Для Android (ARM64)

```bash
# Установите target
rustup target add aarch64-linux-android

# Скомпилируйте
cargo build --target aarch64-linux-android --release
```

### Для iOS (ARM64)

```bash
# Установите target
rustup target add aarch64-apple-ios

# Скомпилируйте
cargo build --target aarch64-apple-ios --release
```

## Ограничения

⚠️ **Текущие ограничения:**
- NAT traversal не реализован (работает только в одной сети)
- Нет автоматического определения внешнего IP
- Нет поддержки UPnP/STUN для обхода NAT

## Планы по улучшению

- [ ] Добавить STUN для определения внешнего IP
- [ ] Реализовать NAT traversal через hole punching
- [ ] Добавить поддержку UPnP
- [ ] Создать мобильные приложения (Android/iOS)
- [ ] Добавить Bluetooth/LoRa для офлайн mesh

## Проверка работы

После запуска двух нод вы должны увидеть:
- Логи о successful handshake
- Логи о encrypted messages
- Heartbeat сообщения каждые 5 секунд
- Peer discovery через UDP

## Отладка

Если соединение не устанавливается:

1. Проверьте firewall на обоих устройствах
2. Убедитесь, что порты открыты
3. Проверьте, что устройства в одной сети
4. Используйте `tcpdump` или Wireshark для анализа трафика

