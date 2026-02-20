#!/usr/bin/env python3
"""
Elysium TUI - Advanced Terminal Interface
Inspired by Claude Code's elegant design
"""
import sys
import asyncio
from datetime import datetime
from typing import Optional, List, Dict, Any

from textual.app import App, ComposeResult
from textual.containers import Container, Horizontal, Vertical, ScrollableContainer
from textual.widgets import (
    Header, Footer, Static, Input,
    DataTable, RichLog, Label, Button
)
from textual.binding import Binding
from textual import events
from textual.reactive import reactive

try:
    from cli import MeshLinkClient, format_inbox_message
except ImportError:
    print("Error: Cannot import MeshLinkClient from cli.py", file=sys.stderr)
    print("Make sure cli.py is in the same directory", file=sys.stderr)
    sys.exit(1)


# Claude Code inspired color scheme
CLAUDE_THEME = """
Screen {
    background: #0d1117;
    color: #c9d1d9;
}

Header {
    background: #161b22;
    color: #f0883e;
    text-style: bold;
}

Footer {
    background: #161b22;
    color: #8b949e;
}

.panel {
    background: #0d1117;
    border: solid #30363d;
    padding: 1;
}

.panel-title {
    color: #f0883e;
    text-style: bold;
}

.status-panel {
    height: 10;
    border: tall #f0883e;
}

.peers-panel {
    height: 12;
    border: tall #30363d;
}

.messages-panel {
    border: tall #30363d;
}

.input-panel {
    height: 5;
    border: tall #f0883e;
}

Input {
    background: #161b22;
    border: none;
    color: #c9d1d9;
}

Input:focus {
    background: #1c2128;
    border: none;
}

DataTable {
    background: #0d1117;
    color: #c9d1d9;
}

DataTable > .datatable--header {
    background: #161b22;
    color: #f0883e;
    text-style: bold;
}

DataTable > .datatable--cursor {
    background: #21262d;
}

RichLog {
    background: #0d1117;
    border: none;
}

Button {
    background: #238636;
    color: white;
    border: none;
}

Button:hover {
    background: #2ea043;
}

.dim {
    color: #8b949e;
}

.success {
    color: #3fb950;
}

.error {
    color: #f85149;
}

.warning {
    color: #d29922;
}

.accent {
    color: #f0883e;
}
"""


class StatusPanel(Static):
    """Panel showing node status"""

    node_id: reactive[str] = reactive("unknown")
    connected_peers: reactive[int] = reactive(0)
    total_peers: reactive[int] = reactive(0)
    api_port: reactive[int] = reactive(0)
    uptime: reactive[str] = reactive("0s")

    def render(self) -> str:
        status = f"""[bold #f0883e]⚡ Elysium Node Status[/bold #f0883e]

[dim]Node ID:[/dim] [#f0883e]{self.node_id[:16]}...[/#f0883e]
[dim]Connected:[/dim] [#3fb950]{self.connected_peers}[/#3fb950][dim]/{self.total_peers}[/dim] peers
[dim]API Port:[/dim] [#f0883e]{self.api_port}[/#f0883e]
[dim]Uptime:[/dim] {self.uptime}
"""
        return status


class PeersPanel(Static):
    """Panel showing connected peers"""

    peers: reactive[List[Dict[str, Any]]] = reactive([])

    def render(self) -> str:
        if not self.peers:
            return "[bold #f0883e]Connected Peers[/bold #f0883e]\n\n[dim]No peers connected[/dim]"

        lines = ["[bold #f0883e]Connected Peers[/bold #f0883e]\n"]
        for i, peer in enumerate(self.peers[:5], 1):  # Show top 5
            node_id = peer.get("id", "unknown")[:12]
            addr = peer.get("address", "unknown")
            state = peer.get("state", "unknown")

            if "Connected" in state:
                status = "[#3fb950]●[/#3fb950]"
            elif "Handshaking" in state or "Connecting" in state:
                status = "[#d29922]●[/#d29922]"
            else:
                status = "[#f85149]●[/#f85149]"

            lines.append(f"{status} [dim]{i}.[/dim] {node_id}...")
            lines.append(f"   [dim]{addr}[/dim]")

        if len(self.peers) > 5:
            lines.append(f"\n[dim]... and {len(self.peers) - 5} more[/dim]")

        return "\n".join(lines)


class MessagesPanel(RichLog):
    """Panel for displaying messages"""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.auto_scroll = True
        self.can_focus = False


class CommandInput(Input):
    """Command input with history"""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.history: List[str] = []
        self.history_index = -1

    async def key_up(self, event: events.Key) -> None:
        """Navigate command history up"""
        if self.history and self.history_index < len(self.history) - 1:
            self.history_index += 1
            self.value = self.history[-(self.history_index + 1)]

    async def key_down(self, event: events.Key) -> None:
        """Navigate command history down"""
        if self.history_index > 0:
            self.history_index -= 1
            self.value = self.history[-(self.history_index + 1)]
        elif self.history_index == 0:
            self.history_index = -1
            self.value = ""


class ElysiumTUI(App):
    """Advanced TUI for Elysium mesh network - Claude Code inspired"""

    CSS = CLAUDE_THEME

    BINDINGS = [
        Binding("ctrl+c", "quit", "Quit", show=True),
        Binding("ctrl+l", "clear", "Clear", show=True),
        Binding("ctrl+r", "refresh", "Refresh", show=True),
        ("f1", "show_help", "Help"),
    ]

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.client: Optional[MeshLinkClient] = None
        self.watch_task: Optional[asyncio.Task] = None
        self.start_time = datetime.now()

    def compose(self) -> ComposeResult:
        """Create the UI layout"""
        yield Header(name="⚡ Elysium - Decentralized Mesh Network")

        with Horizontal():
            # Left column: Status and Peers
            with Vertical(classes="left-column"):
                yield StatusPanel(classes="panel status-panel", id="status")
                yield PeersPanel(classes="panel peers-panel", id="peers")

            # Right column: Messages
            with Vertical(classes="right-column"):
                yield Static("[bold #f0883e]Message Stream[/bold #f0883e]", classes="panel-title")
                yield MessagesPanel(id="messages", classes="panel messages-panel")

        # Bottom: Command input
        with Container(classes="panel input-panel"):
            yield Static("[#f0883e]»[/#f0883e] Command:", classes="dim")
            yield CommandInput(
                placeholder="Type a command (help, send, broadcast, peers, status, clear, quit)...",
                id="command_input"
            )

        yield Footer()

    async def on_mount(self) -> None:
        """Initialize when app starts"""
        messages = self.query_one("#messages", MessagesPanel)

        # Try to connect to node
        messages.write("[#f0883e]═[/#f0883e]" * 50)
        messages.write("[bold #f0883e]⚡ Elysium TUI - Advanced Interface[/bold #f0883e]")
        messages.write("[dim]Inspired by Claude Code[/dim]")
        messages.write("[#f0883e]═[/#f0883e]" * 50)
        messages.write("")

        try:
            self.client = MeshLinkClient()
            messages.write("[#3fb950]✓[/#3fb950] Connected to Elysium node")
            messages.write(f"[dim]API Port: {self.client.api_port}[/dim]")
            messages.write("")
            messages.write("[dim]Type [#f0883e]help[/#f0883e] for available commands[/dim]")
            messages.write("")

            # Update initial status
            await self.update_status()
            await self.update_peers()

            # Start message watcher
            self.watch_task = asyncio.create_task(self.watch_messages())

            # Start status updater
            self.set_interval(5.0, self.update_status)
            self.set_interval(3.0, self.update_peers)

        except Exception as e:
            messages.write(f"[#f85149]✗[/#f85149] Failed to connect: {e}")
            messages.write("[dim]Make sure a node is running: [#f0883e]ely start 8080[/#f0883e][/dim]")

        # Focus command input
        self.query_one("#command_input", CommandInput).focus()

    async def update_status(self) -> None:
        """Update status panel"""
        if not self.client:
            return

        try:
            response = self.client._send_request({"command": "status"})
            if response.get("success"):
                data = response.get("data", {})
                status_panel = self.query_one("#status", StatusPanel)
                status_panel.node_id = data.get("node_id", "unknown")
                status_panel.connected_peers = data.get("connected_peers", 0)
                status_panel.total_peers = data.get("total_peers", 0)
                status_panel.api_port = self.client.api_port

                # Calculate uptime
                uptime_seconds = (datetime.now() - self.start_time).total_seconds()
                if uptime_seconds < 60:
                    status_panel.uptime = f"{int(uptime_seconds)}s"
                elif uptime_seconds < 3600:
                    status_panel.uptime = f"{int(uptime_seconds / 60)}m {int(uptime_seconds % 60)}s"
                else:
                    hours = int(uptime_seconds / 3600)
                    minutes = int((uptime_seconds % 3600) / 60)
                    status_panel.uptime = f"{hours}h {minutes}m"
        except Exception as e:
            pass  # Silently fail status updates

    async def update_peers(self) -> None:
        """Update peers panel"""
        if not self.client:
            return

        try:
            response = self.client._send_request({"command": "peers"})
            if response.get("success"):
                data = response.get("data", {})
                peers_panel = self.query_one("#peers", PeersPanel)
                peers_panel.peers = data.get("peers", [])
        except Exception as e:
            pass  # Silently fail peers updates

    async def watch_messages(self) -> None:
        """Background task to watch for new messages"""
        messages = self.query_one("#messages", MessagesPanel)
        since = 0

        while True:
            try:
                await asyncio.sleep(2)  # Poll every 2 seconds
                if not self.client:
                    break

                since, msgs = self.client.watch(since=since, timeout_ms=5000, limit=50)
                for msg in msgs:
                    formatted = format_inbox_message(msg)
                    messages.write(formatted)
            except Exception as e:
                # Don't spam errors
                break

    async def on_input_submitted(self, event: Input.Submitted) -> None:
        """Handle command submission"""
        command_input = self.query_one("#command_input", CommandInput)
        messages = self.query_one("#messages", MessagesPanel)

        command_line = event.value.strip()
        if not command_line:
            return

        # Add to history
        command_input.history.append(command_line)
        command_input.history_index = -1
        command_input.value = ""

        # Echo command
        messages.write(f"[dim]» {command_line}[/dim]")

        # Parse command
        parts = command_line.split(None, 1)
        command = parts[0].lower() if parts else ""
        args = parts[1] if len(parts) > 1 else ""

        # Execute command
        await self.execute_command(command, args)
        messages.write("")  # Empty line for spacing

    async def execute_command(self, command: str, args: str) -> None:
        """Execute a command"""
        messages = self.query_one("#messages", MessagesPanel)

        if not self.client and command not in ("help", "quit", "exit"):
            messages.write("[#f85149]Error:[/#f85149] Not connected to node")
            return

        if command in ("exit", "quit"):
            await self.action_quit()

        elif command == "help":
            help_text = """[bold #f0883e]Available Commands:[/bold #f0883e]

[#f0883e]send[/#f0883e] <peer_id> <message>
  Send direct message to a specific peer

[#f0883e]broadcast[/#f0883e] <message>
  Broadcast message to all connected peers

[#f0883e]peers[/#f0883e]
  Show all connected peers with details

[#f0883e]status[/#f0883e]
  Show detailed node status

[#f0883e]inbox[/#f0883e] [n]
  Show last N messages from inbox (default: 20)

[#f0883e]clear[/#f0883e]
  Clear message stream

[#f0883e]refresh[/#f0883e]
  Refresh status and peer information

[#f0883e]help[/#f0883e]
  Show this help message

[#f0883e]quit[/#f0883e] or [#f0883e]exit[/#f0883e]
  Exit the application

[bold #f0883e]Keyboard Shortcuts:[/bold #f0883e]
[dim]Ctrl+C[/dim]  - Quit
[dim]Ctrl+L[/dim]  - Clear messages
[dim]Ctrl+R[/dim]  - Refresh
[dim]↑/↓[/dim]     - Command history
"""
            messages.write(help_text)

        elif command == "status":
            await self.update_status()
            status = self.query_one("#status", StatusPanel)
            messages.write(f"[#3fb950]✓[/#3fb950] Status updated")
            messages.write(f"[dim]Node: {status.node_id[:16]}...[/dim]")
            messages.write(f"[dim]Peers: {status.connected_peers}/{status.total_peers}[/dim]")

        elif command == "peers":
            await self.update_peers()
            peers_panel = self.query_one("#peers", PeersPanel)
            if not peers_panel.peers:
                messages.write("[dim]No peers connected[/dim]")
            else:
                messages.write(f"[bold #f0883e]Connected Peers ({len(peers_panel.peers)}):[/bold #f0883e]")
                for i, peer in enumerate(peers_panel.peers, 1):
                    node_id = peer.get("id", "unknown")
                    addr = peer.get("address", "unknown")
                    state = peer.get("state", "unknown")

                    if "Connected" in state:
                        status_icon = "[#3fb950]●[/#3fb950]"
                    else:
                        status_icon = "[#f85149]●[/#f85149]"

                    messages.write(f"{status_icon} [dim]{i}.[/dim] {node_id[:24]}...")
                    messages.write(f"    [dim]{addr} - {state}[/dim]")

        elif command == "send":
            if not args:
                messages.write("[#d29922]Usage:[/#d29922] send <peer_id> <message>")
                return

            send_parts = args.split(None, 1)
            if len(send_parts) < 2:
                messages.write("[#d29922]Usage:[/#d29922] send <peer_id> <message>")
                return

            peer_id = send_parts[0]
            message = send_parts[1]

            if self.client.send_message(peer_id, message):
                messages.write(f"[#3fb950]✓[/#3fb950] Message sent to {peer_id[:12]}...")
            else:
                messages.write(f"[#f85149]✗[/#f85149] Failed to send message")

        elif command == "broadcast":
            if not args:
                messages.write("[#d29922]Usage:[/#d29922] broadcast <message>")
                return

            if self.client.send_message(None, args):
                messages.write(f"[#3fb950]✓[/#3fb950] Broadcast sent to all peers")
            else:
                messages.write(f"[#f85149]✗[/#f85149] Failed to broadcast")

        elif command == "inbox":
            n = 20
            if args.strip().isdigit():
                n = int(args.strip())

            _, msgs = self.client.inbox(0, n)
            if not msgs:
                messages.write("[dim]Inbox is empty[/dim]")
            else:
                messages.write(f"[bold #f0883e]Inbox ({len(msgs)} messages):[/bold #f0883e]")
                for msg in msgs:
                    messages.write(format_inbox_message(msg))

        elif command == "clear":
            await self.action_clear()

        elif command == "refresh":
            await self.action_refresh()

        else:
            messages.write(f"[#f85149]Unknown command:[/#f85149] {command}")
            messages.write("[dim]Type [#f0883e]help[/#f0883e] for available commands[/dim]")

    async def action_quit(self) -> None:
        """Quit the application"""
        if self.watch_task:
            self.watch_task.cancel()
        self.exit()

    async def action_clear(self) -> None:
        """Clear messages"""
        messages = self.query_one("#messages", MessagesPanel)
        messages.clear()
        messages.write("[#3fb950]✓[/#3fb950] Messages cleared")

    async def action_refresh(self) -> None:
        """Refresh status and peers"""
        await self.update_status()
        await self.update_peers()
        messages = self.query_one("#messages", MessagesPanel)
        messages.write("[#3fb950]✓[/#3fb950] Status and peers refreshed")

    async def action_show_help(self) -> None:
        """Show help"""
        await self.execute_command("help", "")


def main():
    """Entry point"""
    app = ElysiumTUI()
    try:
        app.run()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
