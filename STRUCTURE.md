# MeshNet Project Structure

```
meshlink/
├── core/                                  # Основной P2P-протокол (Rust)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                        # Запуск узла
│       ├── lib.rs                         # Общие функции
│       ├── p2p/
│       │   ├── discovery.rs               # LAN/Wi-Fi Direct peer discovery
│       │   ├── encryption.rs              # RSA + AES-GCM encryption
│       │   ├── peer.rs                    # Peer management
│       │   └── protocol.rs                # Protocol definitions
│       ├── ai/
│       │   ├── router.rs                  # AI routing logic
│       │   └── stats_collector.rs         # Statistics collection
│       └── utils/
│           ├── config.rs                  # Configuration
│           ├── crypto.rs                  # Crypto utilities
│           └── logger.rs                  # Logging
│
├── python_cli/                             # CLI для тестирования
│   ├── requirements.txt
│   └── cli.py                              # CLI commands
│
├── web/                                    # Elysium Web
│   ├── backend/
│   │   └── app.py                          # FastAPI backend
│   └── frontend/
│       ├── index.html
│       ├── app.js                           # Dashboard JS
│       └── assets/                          # CSS, JS
│
├── sites/                                  # Локальные сайты
│   ├── site_id_1/
│   └── site_id_2/
│
├── tests/                                  # Тесты
│   ├── core_tests.rs
│   ├── ai_tests.rs
│   └── cli_tests.py
│
├── scripts/                                # Скрипты
│   ├── generate_keys.rs
│   ├── deploy_site.py
│   └── stress_test_network.rs
│
└── docs/                                   # Документация
    ├── architecture.md
    ├── protocol_spec.md
    ├── ai_routing.md
    ├── web_spec.md
    └── roadmap.md
```
