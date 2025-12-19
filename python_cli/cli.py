#!/usr/bin/env python3
"""
MeshNet CLI - Beautiful command-line interface in Claude Code style
Commands: connect, send, peers, status, deploy_site
"""
import json
import socket
import sys
import os
from typing import Optional, Dict, Any
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from rich.text import Text
from rich import box
from rich.prompt import Prompt
from rich.syntax import Syntax
from rich.markdown import Markdown
from rich.align import Align

# Claude Code color scheme
THEME = {
    "bg": "#0d1117",
    "surface": "#161b22",
    "surface_light": "#21262d",
    "text": "#c9d1d9",
    "text_dim": "#8b949e",
    "accent": "#58a6ff",
    "accent_hover": "#79c0ff",
    "success": "#3fb950",
    "warning": "#d29922",
    "error": "#f85149",
    "border": "#30363d",
}

# Default API port (9000 + node_port)
DEFAULT_API_PORTS = [17080, 17081, 17082, 17083, 17084, 17085]

# Initialize rich console with Claude Code theme
console = Console(
    style=THEME["text"],
    force_terminal=True,
    width=120,
)

class MeshLinkClient:
    """Client for connecting to MeshNet node API"""
    
    def __init__(self, api_port: Optional[int] = None):
        self.api_port = api_port or self._discover_api_port()
        if not self.api_port:
            console.print(
                Panel(
                    "[bold red]✗[/bold red] Could not find running node.\n\n"
                    "[dim]Make sure a node is running with:[/dim]\n"
                    "[cyan]cargo run --bin core --release -- <port>[/cyan]",
                    title="[bold]Connection Error[/bold]",
                    border_style="red",
                    box=box.ROUNDED,
                )
            )
            sys.exit(1)
    
    def _discover_api_port(self) -> Optional[int]:
        """Try to find the API port of a running node"""
        # Check environment variable first
        env_port = os.getenv("MESHLINK_API_PORT")
        if env_port:
            try:
                port = int(env_port)
                if self._test_connection(port):
                    return port
            except ValueError:
                pass
        
        # Try common ports
        for port in DEFAULT_API_PORTS:
            if self._test_connection(port):
                return port
        
        return None
    
    def _test_connection(self, port: int) -> bool:
        """Test if API server is listening on port"""
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(0.1)
            result = sock.connect_ex(('127.0.0.1', port))
            sock.close()
            return result == 0
        except:
            return False
    
    def _send_request(self, request: Dict[str, Any]) -> Dict[str, Any]:
        """Send a request to the API server"""
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(5.0)
            sock.connect(('127.0.0.1', self.api_port))
            
            data = json.dumps(request).encode('utf-8')
            sock.sendall(data + b'\n')
            
            response_data = b''
            while True:
                chunk = sock.recv(4096)
                if not chunk:
                    break
                response_data += chunk
                if b'\n' in response_data:
                    break
            
            sock.close()
            
            response = json.loads(response_data.decode('utf-8').strip())
            return response
        except Exception as e:
            return {"success": False, "error": str(e)}
    
    def send_message(self, to: Optional[str], message: str) -> bool:
        """Send a message to a peer or broadcast"""
        if to:
            request = {
                "command": "send",
                "peer_id": to,
                "message": message
            }
        else:
            request = {
                "command": "broadcast",
                "message": message
            }
        
        response = self._send_request(request)
        if not response.get("success", False) or "error" in response:
            console.print(
                Panel(
                    f"[bold red]✗ Error[/bold red]\n\n[dim]{response.get('error', 'Unknown error')}[/dim]",
                    title="[bold]Send Failed[/bold]",
                    border_style="red",
                    box=box.ROUNDED,
                )
            )
            return False
        
        data = response.get("data", {})
        message_id = data.get("message_id", "unknown")
        console.print(
            Panel(
                f"[bold green]✓ Message sent[/bold green]\n\n[dim]Message ID:[/dim] [cyan]{message_id}[/cyan]",
                title="[bold]Success[/bold]",
                border_style="green",
                box=box.ROUNDED,
            )
        )
        return True
    
    def list_peers(self):
        """List all known peers"""
        request = {"command": "peers"}
        response = self._send_request(request)
        
        if not response.get("success", False) or "error" in response:
            console.print(
                Panel(
                    f"[bold red]✗ Error[/bold red]\n\n[dim]{response.get('error', 'Unknown error')}[/dim]",
                    title="[bold]Request Failed[/bold]",
                    border_style="red",
                    box=box.ROUNDED,
                )
            )
            return
        
        data = response.get("data", {})
        peers = data.get("peers", [])
        if not peers:
            console.print(
                Panel(
                    "[yellow]No peers connected[/yellow]\n\n[dim]Wait for peers to connect or start additional nodes.[/dim]",
                    title="[bold]Peers[/bold]",
                    border_style="yellow",
                    box=box.ROUNDED,
                )
            )
            return
        
        # Create beautiful table
        table = Table(
            title="[bold cyan]Connected Peers[/bold cyan]",
            box=box.ROUNDED,
            show_header=True,
            header_style="bold cyan",
            border_style="blue",
            title_style="bold cyan",
        )
        table.add_column("Peer ID", style="cyan", width=36, no_wrap=True)
        table.add_column("Address", style="green", width=20)
        table.add_column("State", justify="center", width=12)
        
        for peer in peers:
            state = peer.get("state", "unknown")
            addr = peer.get("address", "unknown")
            node_id = peer.get("id", "unknown")
            
            # Color code state
            if "Connected" in state:
                state_text = f"[bold green]●[/bold green] [green]Connected[/green]"
            elif "Handshaking" in state or "Connecting" in state:
                state_text = f"[bold yellow]●[/bold yellow] [yellow]Connecting[/yellow]"
            else:
                state_text = f"[bold red]●[/bold red] [red]Disconnected[/red]"
            
            table.add_row(
                f"[cyan]{node_id[:32]}...[/cyan]" if len(node_id) > 32 else f"[cyan]{node_id}[/cyan]",
                f"[green]{addr}[/green]",
                state_text
            )
        
        console.print(table)
        console.print(f"\n[dim]Total: [cyan]{len(peers)}[/cyan] peer(s)[/dim]")
    
    def show_status(self):
        """Show node status"""
        request = {"command": "status"}
        response = self._send_request(request)
        
        if not response.get("success", False) or "error" in response:
            console.print(
                Panel(
                    f"[bold red]✗ Error[/bold red]\n\n[dim]{response.get('error', 'Unknown error')}[/dim]",
                    title="[bold]Request Failed[/bold]",
                    border_style="red",
                    box=box.ROUNDED,
                )
            )
            return
        
        data = response.get("data", {})
        node_id = data.get("node_id", "unknown")
        connected = data.get("connected_peers", 0)
        total = data.get("total_peers", 0)
        
        # Create beautiful status panel
        status_content = f"""
[bold cyan]Node ID:[/bold cyan] [white]{node_id}[/white]

[bold cyan]Connected:[/bold cyan] [bold green]{connected}[/bold green][dim]/[/dim][dim]{total}[/dim] [dim]peers[/dim]

[bold cyan]API Port:[/bold cyan] [yellow]{self.api_port}[/yellow]
        """.strip()
        
        console.print(
            Panel(
                Align.left(Text.from_markup(status_content)),
                title="[bold cyan]⚡ MeshLink Node Status[/bold cyan]",
                border_style="cyan",
                box=box.ROUNDED,
                padding=(1, 2),
            )
        )

def print_welcome():
    """Print welcome message"""
    welcome = """
[bold cyan]╔═══════════════════════════════════════════════════════════╗[/bold cyan]
[bold cyan]║[/bold cyan]  [bold white]⚡ MeshLink CLI[/bold white] - Decentralized P2P Network  [bold cyan]║[/bold cyan]
[bold cyan]╚═══════════════════════════════════════════════════════════╝[/bold cyan]

[dim]Type [cyan]help[/cyan] for commands, [cyan]exit[/cyan] to quit[/dim]
    """.strip()
    console.print(welcome)
    console.print()

def run_repl(client: MeshLinkClient):
    """Run interactive REPL mode"""
    print_welcome()
    
    while True:
        try:
            line = Prompt.ask("[bold cyan]meshlink[/bold cyan] [dim]»[/dim]").strip()
            if not line:
                continue
            
            parts = line.split(None, 1)
            command = parts[0].lower()
            args = parts[1] if len(parts) > 1 else ""
            
            if command == "exit" or command == "quit":
                console.print("\n[bold green]Goodbye![/bold green] [dim]👋[/dim]\n")
                break
            elif command == "help":
                help_content = """
[bold cyan]Available Commands:[/bold cyan]

  [cyan]send[/cyan] <peer_id> <message>
      Send message to specific peer

  [cyan]broadcast[/cyan] <message>
      Broadcast message to all peers

  [cyan]peers[/cyan]
      List all connected peers

  [cyan]status[/cyan]
      Show node status

  [cyan]help[/cyan]
      Show this help message

  [cyan]exit[/cyan]
      Exit interactive mode
                """.strip()
                console.print(Panel(
                    help_content,
                    title="[bold]Help[/bold]",
                    border_style="cyan",
                    box=box.ROUNDED,
                ))
                console.print()
            elif command == "send":
                if not args:
                    console.print("[yellow]Usage: send <peer_id> <message>[/yellow]")
                    continue
                send_parts = args.split(None, 1)
                if len(send_parts) < 2:
                    console.print("[yellow]Usage: send <peer_id> <message>[/yellow]")
                    continue
                peer_id = send_parts[0]
                message = send_parts[1]
                client.send_message(peer_id, message)
            elif command == "broadcast":
                if not args:
                    console.print("[yellow]Usage: broadcast <message>[/yellow]")
                    continue
                client.send_message(None, args)
            elif command == "peers":
                client.list_peers()
            elif command == "status":
                client.show_status()
            else:
                console.print(
                    Panel(
                        f"[yellow]Unknown command: [bold]{command}[/bold][/yellow]\n\n[dim]Type [cyan]help[/cyan] for available commands.[/dim]",
                        title="[bold]Error[/bold]",
                        border_style="yellow",
                        box=box.ROUNDED,
                    )
                )
        except KeyboardInterrupt:
            console.print("\n[bold green]Goodbye![/bold green] [dim]👋[/dim]\n")
            break
        except EOFError:
            console.print("\n[bold green]Goodbye![/bold green] [dim]👋[/dim]\n")
            break
        except Exception as e:
            console.print(
                Panel(
                    f"[bold red]Error:[/bold red] [white]{e}[/white]",
                    title="[bold]Exception[/bold]",
                    border_style="red",
                    box=box.ROUNDED,
                )
            )

def main():
    """Main CLI entry point"""
    # Check for REPL mode
    if len(sys.argv) == 1 or (len(sys.argv) == 2 and sys.argv[1] in ["-i", "--interactive", "repl"]):
        try:
            client = MeshLinkClient()
            run_repl(client)
        except SystemExit:
            pass
        return
    
    if len(sys.argv) < 2:
        console.print(
            Panel(
                "[bold]Usage:[/bold] [cyan]python cli.py[/cyan] <command> [args...]\n"
                "       [cyan]python cli.py[/cyan] [yellow][-i|--interactive|repl][/yellow]  - Interactive mode\n\n"
                "[bold]Commands:[/bold]\n"
                "  [cyan]send[/cyan] <peer_id> <message>  - Send message to specific peer\n"
                "  [cyan]broadcast[/cyan] <message>       - Broadcast message to all peers\n"
                "  [cyan]peers[/cyan]                     - List all peers\n"
                "  [cyan]status[/cyan]                    - Show node status",
                title="[bold]MeshLink CLI[/bold]",
                border_style="cyan",
                box=box.ROUNDED,
            )
        )
        sys.exit(1)
    
    command = sys.argv[1]
    client = MeshLinkClient()
    
    if command == "send":
        if len(sys.argv) < 4:
            console.print("[yellow]Usage: python cli.py send <peer_id> <message>[/yellow]")
            sys.exit(1)
        peer_id = sys.argv[2]
        message = " ".join(sys.argv[3:])
        client.send_message(peer_id, message)
    
    elif command == "broadcast":
        if len(sys.argv) < 3:
            console.print("[yellow]Usage: python cli.py broadcast <message>[/yellow]")
            sys.exit(1)
        message = " ".join(sys.argv[2:])
        client.send_message(None, message)
    
    elif command == "peers":
        client.list_peers()
    
    elif command == "status":
        client.show_status()
    
    else:
        console.print(
            Panel(
                f"[bold red]Unknown command:[/bold red] [white]{command}[/white]",
                title="[bold]Error[/bold]",
                border_style="red",
                box=box.ROUNDED,
            )
        )
        sys.exit(1)

if __name__ == "__main__":
    main()
