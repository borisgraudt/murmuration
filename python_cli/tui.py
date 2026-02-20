#!/usr/bin/env python3
import sys
from typing import Any

# Предполагаем, что ваш cli.py находится рядом, и мы можем импортировать MeshLinkClient
# В идеале, MeshLinkClient стоит вынести в отдельный файл, чтобы его было легко импортировать.
from cli import MeshLinkClient, format_inbox_message

from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, Input, RichLog
from textual.containers import Container

# Цветовая схема, адаптированная для Textual
THEME_CSS = """
Screen {
    background: #0d1117;
    color: #c9d1d9;
}
Header {
    background: #161b22;
}
Footer {
    background: #161b22;
}
Input {
    background: #161b22;
    border: tall #30363d;
}
Input:focus {
    border: tall #d29922;
}
RichLog {
    background: #0d1117;
    border: tall #30363d;
}
"""

class MeshLinkTUI(App):
    """TUI-интерфейс для MeshNet в стиле Claude Code."""

    CSS = THEME_CSS
    BINDINGS = [("ctrl+c", "quit", "Выход")]

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # Инициализируем клиент один раз при старте приложения
        try:
            self.client = MeshLinkClient()
        except SystemExit:
            # Если клиент не смог подключиться, Textual не сможет запуститься.
            # В реальном приложении это нужно обработать изящнее.
            print("Ошибка: не удалось подключиться к узлу MeshNet. Запустите узел и попробуйте снова.", file=sys.stderr)
            sys.exit(1)


    def compose(self) -> ComposeResult:
        """Создание виджетов и макета."""
        yield Header(name="⚡ MeshLink TUI")
        yield RichLog(wrap=True, id="log")
        yield Input(placeholder="Введите команду (help для списка)...", id="command_input")
        yield Footer()

    def on_mount(self) -> None:
        """Вызывается после монтирования всех виджетов."""
        log = self.query_one("#log", RichLog)
        log.write("[yellow]╔═══════════════════════════════════╗[/yellow]")
        log.write("[yellow]║[/yellow]  [white]⚡ MeshLink TUI[/white] - P2P Network  [yellow]║[/yellow]")
        log.write("[yellow]╚═══════════════════════════════════╝[/yellow]")
        log.write("[dim]Введите [yellow]help[/yellow] для списка команд, [yellow]exit[/yellow] для выхода[/dim]\n")
        self.query_one("#command_input", Input).focus()
        
        # Запускаем фоновую задачу для 'watch'
        self.run_worker(self.watch_messages, thread=True)

    async def on_input_submitted(self, message: Input.Submitted) -> None:
        """Обработка отправки команды из поля ввода."""
        log = self.query_one("#log", RichLog)
        command_line = message.value
        input_widget = self.query_one("#command_input", Input)
        input_widget.clear() # Очищаем поле ввода

        log.write(f"[dim]» {command_line}[/dim]")

        parts = command_line.strip().split(None, 1)
        command = parts[0].lower() if parts else ""
        args = parts[1] if len(parts) > 1 else ""

        # --- Обработка команд ---
        if command in ("exit", "quit"):
            self.exit()
        elif command == "help":
            help_text = """
[yellow]Available Commands:[/yellow]
  [yellow]send[/yellow] <peer_id> <message>
  [yellow]broadcast[/yellow] <message>
  [yellow]peers[/yellow]
  [yellow]status[/yellow]
  [yellow]inbox[/yellow] [n]
  [yellow]clear[/yellow] - Очистить лог
  [yellow]exit[/yellow]
            """
            log.write(help_text)
        elif command == "status":
            # В Textual методы, делающие I/O, лучше делать асинхронными
            # Но для простоты примера оставим синхронными
            status_data = self.client._send_request({"command": "status"})
            # Здесь вы бы отформатировали ответ и вывели в лог
            log.write(f"[green]Status:[/green] {status_data.get('data', {})}")
        elif command == "peers":
            peers_data = self.client._send_request({"command": "peers"})
            peers = peers_data.get("data", {}).get("peers", [])
            if not peers:
                log.write("[dim]No peers connected.[/dim]")
            else:
                log.write(f"[yellow]Connected Peers ({len(peers)}):[/yellow]")
                for peer in peers:
                    log.write(f"  - {peer.get('id', 'N/A')[:12]}... ({peer.get('address', '?')})")
        elif command == "clear":
            log.clear()
        elif command:
            log.write(f"[red]Неизвестная команда:[/red] {command}")
        
        log.write("") # Пустая строка для отступа

    def watch_messages(self) -> None:
        """Фоновая задача, которая слушает сообщения (как `watch`)."""
        log = self.query_one("#log", RichLog)
        since = 0
        while True:
            # Используем call_from_thread, чтобы безопасно обновить виджет из другого потока
            try:
                since, msgs = self.client.watch(since=since, timeout_ms=30000, limit=10)
                if msgs:
                    for m in msgs:
                        # self.call_from_thread(log.write, format_inbox_message(m)) # Textual < 0.50
                        self.post_message(log.write(format_inbox_message(m))) # Textual >= 0.50
            except Exception:
                # self.call_from_thread(log.write, "[red]Ошибка 'watch' потока.[/red]")
                self.post_message(log.write("[red]Ошибка 'watch' потока.[/red]"))
                break


if __name__ == "__main__":
    app = MeshLinkTUI()
    app.run()

