"""
Python CLI tests
"""
import unittest
import sys
from unittest.mock import Mock, patch, MagicMock
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent / "python_cli"))

from cli import MeshLinkClient


class TestMeshLinkClient(unittest.TestCase):
    """Test MeshLinkClient class"""

    @patch('cli.socket.socket')
    def test_test_connection_success(self, mock_socket):
        """Test successful connection test"""
        mock_sock = MagicMock()
        mock_sock.connect_ex.return_value = 0
        mock_socket.return_value = mock_sock
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        result = client._test_connection(17082)
        
        self.assertTrue(result)
        mock_sock.connect_ex.assert_called_once_with(('127.0.0.1', 17082))
        mock_sock.close.assert_called_once()

    @patch('cli.socket.socket')
    def test_test_connection_failure(self, mock_socket):
        """Test failed connection test"""
        mock_sock = MagicMock()
        mock_sock.connect_ex.return_value = 1
        mock_socket.return_value = mock_sock
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        result = client._test_connection(17082)
        
        self.assertFalse(result)

    @patch('cli.MeshLinkClient._test_connection')
    @patch('cli.os.getenv')
    def test_discover_api_port_from_env(self, mock_getenv, mock_test_conn):
        """Test API port discovery from environment variable"""
        mock_getenv.return_value = "17082"
        mock_test_conn.return_value = True
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        port = client._discover_api_port()
        
        self.assertEqual(port, 17082)
        mock_getenv.assert_called_once_with("MESHLINK_API_PORT")

    @patch('cli.MeshLinkClient._test_connection')
    @patch('cli.os.getenv')
    def test_discover_api_port_from_defaults(self, mock_getenv, mock_test_conn):
        """Test API port discovery from default ports"""
        mock_getenv.return_value = None
        mock_test_conn.side_effect = [False, False, True]  # First two fail, third succeeds
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        port = client._discover_api_port()
        
        self.assertEqual(port, 17082)  # Third port in DEFAULT_API_PORTS
        self.assertEqual(mock_test_conn.call_count, 3)

    @patch('cli.socket.socket')
    @patch('json.loads')
    @patch('json.dumps')
    def test_send_request_success(self, mock_dumps, mock_loads, mock_socket):
        """Test successful request sending"""
        mock_sock = MagicMock()
        mock_sock.recv.side_effect = [b'{"success": true, "data": {"test": "value"}}\n', b'']
        mock_socket.return_value = mock_sock
        mock_dumps.return_value = '{"command": "test"}'
        mock_loads.return_value = {"success": True, "data": {"test": "value"}}
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        client.api_port = 17082
        
        request = {"command": "test"}
        response = client._send_request(request)
        
        self.assertTrue(response.get("success"))
        mock_sock.connect.assert_called_once_with(('127.0.0.1', 17082))
        mock_sock.sendall.assert_called_once()
        mock_sock.close.assert_called_once()

    @patch('cli.socket.socket')
    def test_send_request_connection_error(self, mock_socket):
        """Test request sending with connection error"""
        mock_sock = MagicMock()
        mock_sock.connect.side_effect = Exception("Connection refused")
        mock_socket.return_value = mock_sock
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        client.api_port = 17082
        
        request = {"command": "test"}
        response = client._send_request(request)
        
        self.assertFalse(response.get("success", True))
        self.assertIn("error", response)


class TestCLICommands(unittest.TestCase):
    """Test CLI command execution"""

    @patch('cli.MeshLinkClient._send_request')
    def test_send_message_success(self, mock_send_request):
        """Test successful message sending"""
        mock_send_request.return_value = {
            "success": True,
            "data": {"message_id": "test-123"}
        }
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        client.api_port = 17082
        
        result = client.send_message("peer-123", "Hello")
        
        self.assertTrue(result)
        mock_send_request.assert_called_once()
        call_args = mock_send_request.call_args[0][0]
        self.assertEqual(call_args["command"], "send")
        self.assertEqual(call_args["peer_id"], "peer-123")
        self.assertEqual(call_args["message"], "Hello")

    @patch('cli.MeshLinkClient._send_request')
    def test_broadcast_message_success(self, mock_send_request):
        """Test successful broadcast message"""
        mock_send_request.return_value = {
            "success": True,
            "data": {"message_id": "test-456"}
        }
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        client.api_port = 17082
        
        result = client.send_message(None, "Broadcast message")
        
        self.assertTrue(result)
        mock_send_request.assert_called_once()
        call_args = mock_send_request.call_args[0][0]
        self.assertEqual(call_args["command"], "broadcast")
        self.assertEqual(call_args["message"], "Broadcast message")

    @patch('cli.MeshLinkClient._send_request')
    def test_list_peers_success(self, mock_send_request):
        """Test successful peers listing"""
        mock_send_request.return_value = {
            "success": True,
            "data": {
                "peers": [
                    {"id": "peer-1", "address": "127.0.0.1:8082", "state": "Connected"},
                    {"id": "peer-2", "address": "127.0.0.1:8083", "state": "Connected"}
                ]
            }
        }
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        client.api_port = 17082
        
        # Capture print output
        import io
        from contextlib import redirect_stdout
        
        f = io.StringIO()
        with redirect_stdout(f):
            client.list_peers()
        
        output = f.getvalue()
        self.assertIn("peer-1", output)
        self.assertIn("peer-2", output)

    @patch('cli.MeshLinkClient._send_request')
    def test_show_status_success(self, mock_send_request):
        """Test successful status display"""
        mock_send_request.return_value = {
            "success": True,
            "data": {
                "node_id": "test-node-123",
                "connected_peers": 2,
                "total_peers": 3
            }
        }
        
        client = MeshLinkClient.__new__(MeshLinkClient)
        client.api_port = 17082
        
        # Capture print output
        import io
        from contextlib import redirect_stdout
        
        f = io.StringIO()
        with redirect_stdout(f):
            client.show_status()
        
        output = f.getvalue()
        self.assertIn("test-node-123", output)
        self.assertIn("2/3", output)


if __name__ == "__main__":
    unittest.main()
