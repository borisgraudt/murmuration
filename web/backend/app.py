"""
Elysium Web Backend - FastAPI server for web interface
Provides API for sites, chat, peer statistics, and integration with Rust core
"""
from fastapi import FastAPI, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
from pydantic import BaseModel
from typing import Optional, List, Dict
import json
import socket
import os

app = FastAPI(title="Elysium Web Backend")

# Configuration
RUST_API_PORT = int(os.getenv("MESHLINK_API_PORT", "17080"))
SITES_DIR = os.path.join(os.path.dirname(__file__), "../../sites")

class MessageRequest(BaseModel):
    to: Optional[str] = None
    message: str

class SiteRequest(BaseModel):
    site_id: str
    content: Optional[str] = None

def call_rust_api(command: str, args: Dict = None) -> Dict:
    """Call Rust node API"""
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(5.0)
        sock.connect(('127.0.0.1', RUST_API_PORT))
        
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
        return json.loads(response_data.decode('utf-8').strip())
    except Exception as e:
        return {"error": str(e)}

@app.get("/")
async def root():
    """Root endpoint"""
    return {"message": "Elysium Web Backend", "version": "0.1.0"}

@app.get("/api/peers")
async def get_peers():
    """Get list of peers"""
    response = call_rust_api("peers")
    if "error" in response:
        raise HTTPException(status_code=500, detail=response["error"])
    return response

@app.get("/api/status")
async def get_status():
    """Get node status"""
    response = call_rust_api("status")
    if "error" in response:
        raise HTTPException(status_code=500, detail=response["error"])
    return response

@app.post("/api/send")
async def send_message(req: MessageRequest):
    """Send a message"""
    command = "send" if req.to else "broadcast"
    args = {"message": req.message}
    if req.to:
        args["to"] = req.to
    
    response = call_rust_api(command, args)
    if "error" in response:
        raise HTTPException(status_code=500, detail=response["error"])
    return response

@app.get("/api/sites")
async def list_sites():
    """List available sites"""
    sites = []
    if os.path.exists(SITES_DIR):
        for item in os.listdir(SITES_DIR):
            site_path = os.path.join(SITES_DIR, item)
            if os.path.isdir(site_path):
                sites.append({
                    "site_id": item,
                    "path": f"/sites/{item}/"
                })
    return {"sites": sites}

@app.get("/sites/{site_id}/")
async def get_site(site_id: str):
    """Get site index"""
    site_path = os.path.join(SITES_DIR, site_id, "index.html")
    if os.path.exists(site_path):
        return FileResponse(site_path)
    raise HTTPException(status_code=404, detail="Site not found")

@app.get("/sites/{site_id}/{filename}")
async def get_site_file(site_id: str, filename: str):
    """Get site file"""
    site_path = os.path.join(SITES_DIR, site_id, filename)
    if os.path.exists(site_path) and os.path.isfile(site_path):
        return FileResponse(site_path)
    raise HTTPException(status_code=404, detail="File not found")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)

