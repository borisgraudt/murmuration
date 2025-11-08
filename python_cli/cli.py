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

# Default API port (9000 + node_port)
DEFAULT_API_PORTS = [17080, 17081, 17082, 17083, 17084, 17085]

class MeshLinkClient:
    """Client for connecting to MeshNet node API"""
    
    def __init__(self, api_port: Optional[int] = None):
        self.api_port = api_port or self._discover_api_port()
        if not self.api_port:
            print("Error: Could not find running node. Make sure a node is running.")
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
    
    def _send_request(self, command: str, args: Dict[str, Any] = None) -> Dict[str, Any]:
        """Send a request to the API server"""
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(5.0)
            sock.connect(('127.0.0.1', self.api_port))
            
            request = {
                "command": command,
                "args": args or {}
            }
            
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
            return {"error": str(e)}
    
    def send_message(self, to: Optional[str], message: str) -> bool:
        """Send a message to a peer or broadcast"""
        args = {"message": message}
        if to:
            args["to"] = to
        
        response = self._send_request("send" if to else "broadcast", args)
        if "error" in response:
            print(f"Error: {response['error']}")
            return False
        
        print(f"Message sent: {response.get('message_id', 'unknown')}")
        return True
    
    def list_peers(self):
        """List all known peers"""
        response = self._send_request("peers")
        if "error" in response:
            print(f"Error: {response['error']}")
            return
        
        peers = response.get("peers", [])
        if not peers:
            print("No peers connected")
            return
        
        print(f"\nPeers ({len(peers)}):")
        print("-" * 60)
        for peer in peers:
            state = peer.get("state", "unknown")
            addr = peer.get("address", "unknown")
            node_id = peer.get("node_id", "unknown")
            print(f"{node_id[:8]}... @ {addr} [{state}]")
    
    def show_status(self):
        """Show node status"""
        response = self._send_request("status")
        if "error" in response:
            print(f"Error: {response['error']}")
            return
        
        node_id = response.get("node_id", "unknown")
        connected = response.get("connected_peers", 0)
        total = response.get("total_peers", 0)
        
        print(f"\nNode Status:")
        print("-" * 60)
        print(f"Node ID: {node_id}")
        print(f"Connected: {connected}/{total} peers")
        print(f"API Port: {self.api_port}")

def main():
    """Main CLI entry point"""
    if len(sys.argv) < 2:
        print("Usage: python cli.py <command> [args...]")
        print("\nCommands:")
        print("  send <peer_id> <message>  - Send message to specific peer")
        print("  broadcast <message>       - Broadcast message to all peers")
        print("  peers                     - List all peers")
        print("  status                    - Show node status")
        sys.exit(1)
    
    command = sys.argv[1]
    client = MeshLinkClient()
    
    if command == "send":
        if len(sys.argv) < 4:
            print("Usage: python cli.py send <peer_id> <message>")
            sys.exit(1)
        peer_id = sys.argv[2]
        message = " ".join(sys.argv[3:])
        client.send_message(peer_id, message)
    
    elif command == "broadcast":
        if len(sys.argv) < 3:
            print("Usage: python cli.py broadcast <message>")
            sys.exit(1)
        message = " ".join(sys.argv[2:])
        client.send_message(None, message)
    
    elif command == "peers":
        client.list_peers()
    
    elif command == "status":
        client.show_status()
    
    else:
        print(f"Unknown command: {command}")
        sys.exit(1)

if __name__ == "__main__":
    main()

