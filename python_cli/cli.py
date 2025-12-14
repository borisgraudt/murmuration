#!/usr/bin/env python3
"""
MeshNet CLI - Command-line interface for testing the P2P network
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
from rich.live import Live
from rich.layout import Layout
from rich.align import Align

# Default API port (9000 + node_port)
DEFAULT_API_PORTS = [17080, 17081, 17082, 17083, 17084, 17085]

# Initialize rich console
console = Console()

class MeshLinkClient:
    """Client for connecting to MeshNet node API"""
    
    def __init__(self, api_port: Optional[int] = None):
        self.api_port = api_port or self._discover_api_port()
        if not self.api_port:
            console.print("[bold red]✗[/bold red] Could not find running node. Make sure a node is running.", style="red")
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
            console.print(f"[bold red]✗ Error:[/bold red] {response.get('error', 'Unknown error')}", style="red")
            return False
        
        data = response.get("data", {})
        message_id = data.get("message_id", "unknown")
        console.print(f"[bold green]✓[/bold green] Message sent: [cyan]{message_id}[/cyan]")
        return True
    
    def list_peers(self):
        """List all known peers"""
        request = {"command": "peers"}
        response = self._send_request(request)
        
        if not response.get("success", False) or "error" in response:
            console.print(f"[bold red]✗ Error:[/bold red] {response.get('error', 'Unknown error')}", style="red")
            return
        
        data = response.get("data", {})
        peers = data.get("peers", [])
        if not peers:
            console.print("[yellow]No peers connected[/yellow]")
            return
        
        table = Table(title="[bold cyan]Connected Peers[/bold cyan]", box=box.ROUNDED, show_header=True, header_style="bold magenta")
        table.add_column("Peer ID", style="cyan", width=40)
        table.add_column("Address", style="green")
        table.add_column("State", justify="center")
        
        for peer in peers:
            state = peer.get("state", "unknown")
            addr = peer.get("address", "unknown")
            node_id = peer.get("id", "unknown")
            
            # Color code state
            state_style = "green" if "Connected" in state else "yellow" if "Connecting" in state else "red"
            state_text = f"[{state_style}]{state}[/{state_style}]"
            
            table.add_row(node_id, addr, state_text)
        
        console.print(table)
        console.print(f"\n[dim]Total: {len(peers)} peer(s)[/dim]")
    
    def show_status(self):
        """Show node status"""
        request = {"command": "status"}
        response = self._send_request(request)
        
        if not response.get("success", False) or "error" in response:
            console.print(f"[bold red]✗ Error:[/bold red] {response.get('error', 'Unknown error')}", style="red")
            return
        
        data = response.get("data", {})
        node_id = data.get("node_id", "unknown")
        connected = data.get("connected_peers", 0)
        total = data.get("total_peers", 0)
        
        # Create status panel
        status_text = f"""
[bold cyan]Node ID:[/bold cyan] {node_id}
[bold cyan]Connected:[/bold cyan] [green]{connected}[/green]/[dim]{total}[/dim] peers
[bold cyan]API Port:[/bold cyan] [yellow]{self.api_port}[/yellow]
        """.strip()
        
        panel = Panel(
            Align.left(Text.from_markup(status_text)),
            title="[bold green]⚡ MeshLink Node Status[/bold green]",
            border_style="cyan",
            box=box.ROUNDED
        )
        console.print(panel)

def run_repl(client: MeshLinkClient):
    """Run interactive REPL mode"""
    console.print(Panel(
        "[bold cyan]⚡ MeshLink CLI - Interactive Mode[/bold cyan]\n[dim]Type 'help' for commands, 'exit' to quit[/dim]",
        border_style="cyan",
        box=box.ROUNDED
    ))
    
    while True:
        try:
            line = Prompt.ask("[bold cyan]meshlink[/bold cyan]").strip()
            if not line:
                continue
            
            parts = line.split(None, 1)
            command = parts[0].lower()
            args = parts[1] if len(parts) > 1 else ""
            
            if command == "exit" or command == "quit":
                console.print("[bold green]Goodbye![/bold green]")
                break
            elif command == "help":
                help_table = Table(box=box.ROUNDED, show_header=False, padding=(0, 2))
                help_table.add_column("Command", style="cyan")
                help_table.add_column("Description", style="white")
                
                help_table.add_row("send <peer_id> <message>", "Send message to specific peer")
                help_table.add_row("broadcast <message>", "Broadcast message to all peers")
                help_table.add_row("peers", "List all peers")
                help_table.add_row("status", "Show node status")
                help_table.add_row("help", "Show this help")
                help_table.add_row("exit", "Exit interactive mode")
                
                console.print("\n[bold]Available Commands:[/bold]")
                console.print(help_table)
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
                console.print(f"[yellow]Unknown command: {command}. Type 'help' for available commands.[/yellow]")
        except KeyboardInterrupt:
            console.print("\n[bold green]Goodbye![/bold green]")
            break
        except EOFError:
            console.print("\n[bold green]Goodbye![/bold green]")
            break
        except Exception as e:
            console.print(f"[bold red]Error:[/bold red] {e}", style="red")

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
        console.print("[bold red]Usage:[/bold red] python cli.py <command> [args...]")
        console.print("       python cli.py [-i|--interactive|repl]  - Interactive mode")
        console.print("\n[bold]Commands:[/bold]")
        console.print("  [cyan]send[/cyan] <peer_id> <message>  - Send message to specific peer")
        console.print("  [cyan]broadcast[/cyan] <message>       - Broadcast message to all peers")
        console.print("  [cyan]peers[/cyan]                     - List all peers")
        console.print("  [cyan]status[/cyan]                    - Show node status")
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
        console.print(f"[bold red]Unknown command:[/bold red] {command}")
        sys.exit(1)

if __name__ == "__main__":
    main()

