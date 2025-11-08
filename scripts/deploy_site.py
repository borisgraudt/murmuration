#!/usr/bin/env python3
"""
Script to deploy a site to the MeshNet network
"""
import os
import sys
import json
import socket
from pathlib import Path

def deploy_site(site_path: str, site_id: str, api_port: int = 17080):
    """Deploy a site to the mesh network"""
    site_path = Path(site_path)
    if not site_path.exists():
        print(f"Error: Site path {site_path} does not exist")
        return False
    
    # Read site files
    site_files = {}
    for file_path in site_path.rglob("*"):
        if file_path.is_file():
            rel_path = file_path.relative_to(site_path)
            with open(file_path, 'rb') as f:
                site_files[str(rel_path)] = f.read().decode('utf-8', errors='ignore')
    
    # TODO: Implement actual site deployment protocol
    # This would send GET_SITE/SEND_SITE messages to peers
    print(f"Deploying site {site_id} from {site_path}")
    print(f"Files: {list(site_files.keys())}")
    print("Site deployment protocol not yet implemented")
    
    return True

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python deploy_site.py <site_path> <site_id> [api_port]")
        sys.exit(1)
    
    site_path = sys.argv[1]
    site_id = sys.argv[2]
    api_port = int(sys.argv[3]) if len(sys.argv) > 3 else 17080
    
    deploy_site(site_path, site_id, api_port)

