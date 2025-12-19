#!/usr/bin/env python3
"""
MeshNet CLI - Claude Code v2.0 style interface
Commands: connect, send, peers, status, deploy_site
"""
import json
import socket
import sys
import os
from typing import Optional, Dict, Any
from rich.console import Console
from rich.panel import Panel
from rich.text import Text
from rich import box
from rich.prompt import Prompt
from rich.align import Align

# Claude Code v2.0 color scheme (orange accents)
THEME = {
    "bg": "#0d1117",
    "surface": "#161b22",
    "text": "#c9d1d9",
    "text_dim": "#8b949e",
    "accent": "#d29922",  # Orange like Claude Code
    "accent_light": "#f0883e",
    "success": "#3fb950",
    "error": "#f85149",
    "border": "#30363d",
}

# Default API port (9000 + node_port)
DEFAULT_API_PORTS = [17080, 17081, 17082, 17083, 17084, 17085]

# Initialize rich console
console = Console(
    style=THEME["text"],
    force_terminal=True,
    width=100,
)

class MeshLinkClient:
    """Client for connecting to MeshNet node API"""
    
    def __init__(self, api_port: Optional[int] = None):
        self.api_port = api_port or self._discover_api_port()
        if not self.api_port:
            self._show_error("Could not find running node", 
                           "Make sure a node is running:\ncargo run --bin core --release -- <port>")
            sys.exit(1)
    
    def _discover_api_port(self) -> Optional[int]:
        """Try to find the API port of a running node"""
        env_port = os.getenv("MESHLINK_API_PORT")
        if env_port:
            try:
                port = int(env_port)
                if self._test_connection(port):
                    return port
            except ValueError:
                pass
        
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
    
    def _show_error(self, title: str, message: str):
        """Show error in Claude Code style"""
        console.print(
            Panel(
                f"[{THEME['error']}]{message}[/{THEME['error']}]",
                title=f"[{THEME['accent']}]{title}[/{THEME['accent']}]",
                border_style=THEME["accent"],
                box=box.ROUNDED,
            )
        )
    
    def send_message(self, to: Optional[str], message: str) -> bool:
        """Send a message to a peer or broadcast"""
        if to:
            request = {"command": "send", "peer_id": to, "message": message}
        else:
            request = {"command": "broadcast", "message": message}
        
        response = self._send_request(request)
        if not response.get("success", False) or "error" in response:
            self._show_error("Send failed", response.get('error', 'Unknown error'))
            return False
        
        data = response.get("data", {})
        message_id = data.get("message_id", "unknown")
        console.print(f"[{THEME['success']}]✓ Message sent: [{THEME['accent']}]{message_id}[/{THEME['accent']}][/{THEME['success']}]")
        return True
    
    def list_peers(self):
        """List all known peers"""
        request = {"command": "peers"}
        response = self._send_request(request)
        
        if not response.get("success", False) or "error" in response:
            self._show_error("Request failed", response.get('error', 'Unknown error'))
            return
        
        data = response.get("data", {})
        peers = data.get("peers", [])
        
        if not peers:
            console.print(f"[{THEME['text_dim']}]No peers connected[/{THEME['text_dim']}]")
            return
        
        # Claude Code style peer list
        peer_lines = []
        for peer in peers:
            state = peer.get("state", "unknown")
            addr = peer.get("address", "unknown")
            node_id = peer.get("id", "unknown")
            
            if "Connected" in state:
                status = f"[{THEME['success']}]● Connected[/{THEME['success']}]"
            elif "Handshaking" in state or "Connecting" in state:
                status = f"[{THEME['accent']}]● Connecting[/{THEME['accent']}]"
            else:
                status = f"[{THEME['error']}]● Disconnected[/{THEME['error']}]"
            
            peer_lines.append(f"[{THEME['text']}]{node_id[:36]}...[/{THEME['text']}]")
            peer_lines.append(f"  [{THEME['text_dim']}]{addr}[/{THEME['text_dim']}] {status}")
        
        content = "\n".join(peer_lines)
        console.print(
            Panel(
                content,
                title=f"[{THEME['accent']}]Connected Peers[/{THEME['accent']}]",
                border_style=THEME["accent"],
                box=box.ROUNDED,
            )
        )
        console.print(f"[{THEME['text_dim']}]Total: {len(peers)} peer(s)[/{THEME['text_dim']}]")
    
    def show_status(self):
        """Show node status in Claude Code style"""
        request = {"command": "status"}
        response = self._send_request(request)
        
        if not response.get("success", False) or "error" in response:
            self._show_error("Request failed", response.get('error', 'Unknown error'))
            return
        
        data = response.get("data", {})
        node_id = data.get("node_id", "unknown")
        connected = data.get("connected_peers", 0)
        total = data.get("total_peers", 0)
        
        # Check for protocol errors
        has_peers = connected > 0
        
        # Claude Code style status panel
        status_content = f"""
[{THEME['text']}]Node ID:[/{THEME['text']}] [{THEME['accent']}]{node_id}[/{THEME['accent']}]

[{THEME['text']}]Connected:[/{THEME['text']}] [{THEME['success']}]{connected}[/{THEME['success']}][{THEME['text_dim']}]/{total}[/{THEME['text_dim']}] [{THEME['text_dim']}]peers[/{THEME['text_dim']}]

[{THEME['text']}]API Port:[/{THEME['text']}] [{THEME['accent']}]{self.api_port}[/{THEME['accent']}]
        """.strip()
        
        if not has_peers:
            status_content += f"\n\n[{THEME['accent']}]⚠ Protocol error: No connected peers[/{THEME['accent']}]"
        
        console.print(
            Panel(
                Align.left(Text.from_markup(status_content)),
                title=f"[{THEME['accent']}]⚡ MeshLink Node Status[/{THEME['accent']}]",
                border_style=THEME["accent"],
                box=box.ROUNDED,
                padding=(1, 2),
            )
        )

def print_welcome():
    """Print welcome message in Claude Code style"""
    welcome = f"""
[{THEME['accent']}]╔═══════════════════════════════════════════════════════════╗[/{THEME['accent']}]
[{THEME['accent']}]║[/{THEME['accent']}]  [{THEME['text']}]⚡ MeshLink CLI[/{THEME['text']}] - Decentralized P2P Network  [{THEME['accent']}]║[/{THEME['accent']}]
[{THEME['accent']}]╚═══════════════════════════════════════════════════════════╝[/{THEME['accent']}]

[{THEME['text_dim']}]Type [{THEME['accent']}]help[/{THEME['accent']}] for commands, [{THEME['accent']}]exit[/{THEME['accent']}] to quit[/{THEME['text_dim']}]
    """.strip()
    console.print(welcome)
    console.print()

def run_repl(client: MeshLinkClient):
    """Run interactive REPL mode"""
    print_welcome()
    
    while True:
        try:
            line = Prompt.ask(f"[{THEME['accent']}]meshlink[/{THEME['accent']}] [{THEME['text_dim']}]»[/{THEME['text_dim']}]").strip()
            if not line:
                continue
            
            parts = line.split(None, 1)
            command = parts[0].lower()
            args = parts[1] if len(parts) > 1 else ""
            
            if command == "exit" or command == "quit":
                console.print(f"\n[{THEME['success']}]Goodbye![/{THEME['success']}] [{THEME['text_dim']}]👋[/{THEME['text_dim']}]\n")
                break
            elif command == "help":
                help_content = f"""
[{THEME['accent']}]Available Commands:[/{THEME['accent']}]

  [{THEME['accent']}]send[/{THEME['accent']}] <peer_id> <message>
      Send message to specific peer

  [{THEME['accent']}]broadcast[/{THEME['accent']}] <message>
      Broadcast message to all peers

  [{THEME['accent']}]peers[/{THEME['accent']}]
      List all connected peers

  [{THEME['accent']}]status[/{THEME['accent']}]
      Show node status

  [{THEME['accent']}]help[/{THEME['accent']}]
      Show this help message

  [{THEME['accent']}]exit[/{THEME['accent']}]
      Exit interactive mode
                """.strip()
                console.print(Panel(
                    help_content,
                    title=f"[{THEME['accent']}]Help[/{THEME['accent']}]",
                    border_style=THEME["accent"],
                    box=box.ROUNDED,
                ))
                console.print()
            elif command == "send":
                if not args:
                    console.print(f"[{THEME['accent']}]Usage: send <peer_id> <message>[/{THEME['accent']}]")
                    continue
                send_parts = args.split(None, 1)
                if len(send_parts) < 2:
                    console.print(f"[{THEME['accent']}]Usage: send <peer_id> <message>[/{THEME['accent']}]")
                    continue
                peer_id = send_parts[0]
                message = send_parts[1]
                client.send_message(peer_id, message)
            elif command == "broadcast":
                if not args:
                    console.print(f"[{THEME['accent']}]Usage: broadcast <message>[/{THEME['accent']}]")
                    continue
                client.send_message(None, args)
            elif command == "peers":
                client.list_peers()
            elif command == "status":
                client.show_status()
            else:
                console.print(
                    Panel(
                        f"[{THEME['accent']}]Unknown command: [{THEME['text']}]{command}[/{THEME['text']}][/{THEME['accent']}]\n\n[{THEME['text_dim']}]Type [{THEME['accent']}]help[/{THEME['accent']}] for available commands.[/{THEME['text_dim']}]",
                        title=f"[{THEME['accent']}]Error[/{THEME['accent']}]",
                        border_style=THEME["accent"],
                        box=box.ROUNDED,
                    )
                )
        except KeyboardInterrupt:
            console.print(f"\n[{THEME['success']}]Goodbye![/{THEME['success']}] [{THEME['text_dim']}]👋[/{THEME['text_dim']}]\n")
            break
        except EOFError:
            console.print(f"\n[{THEME['success']}]Goodbye![/{THEME['success']}] [{THEME['text_dim']}]👋[/{THEME['text_dim']}]\n")
            break
        except Exception as e:
            client._show_error("Exception", str(e))

def main():
    """Main CLI entry point"""
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
                f"[{THEME['text']}]Usage:[/{THEME['text']}] [{THEME['accent']}]python cli.py[/{THEME['accent']}] <command> [args...]\n"
                f"       [{THEME['accent']}]python cli.py[/{THEME['accent']}] [{THEME['text_dim']}][-i|--interactive|repl][/{THEME['text_dim']}]  - Interactive mode\n\n"
                f"[{THEME['text']}]Commands:[/{THEME['text']}]\n"
                f"  [{THEME['accent']}]send[/{THEME['accent']}] <peer_id> <message>  - Send message to specific peer\n"
                f"  [{THEME['accent']}]broadcast[/{THEME['accent']}] <message>       - Broadcast message to all peers\n"
                f"  [{THEME['accent']}]peers[/{THEME['accent']}]                     - List all peers\n"
                f"  [{THEME['accent']}]status[/{THEME['accent']}]                    - Show node status",
                title=f"[{THEME['accent']}]MeshLink CLI[/{THEME['accent']}]",
                border_style=THEME["accent"],
                box=box.ROUNDED,
            )
        )
        sys.exit(1)
    
    command = sys.argv[1]
    client = MeshLinkClient()
    
    if command == "send":
        if len(sys.argv) < 4:
            console.print(f"[{THEME['accent']}]Usage: python cli.py send <peer_id> <message>[/{THEME['accent']}]")
            sys.exit(1)
        peer_id = sys.argv[2]
        message = " ".join(sys.argv[3:])
        client.send_message(peer_id, message)
    
    elif command == "broadcast":
        if len(sys.argv) < 3:
            console.print(f"[{THEME['accent']}]Usage: python cli.py broadcast <message>[/{THEME['accent']}]")
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
                f"[{THEME['error']}]Unknown command:[/{THEME['error']}] [{THEME['text']}]{command}[/{THEME['text']}]",
                title=f"[{THEME['accent']}]Error[/{THEME['accent']}]",
                border_style=THEME["accent"],
                box=box.ROUNDED,
            )
        )
        sys.exit(1)

if __name__ == "__main__":
    main()
