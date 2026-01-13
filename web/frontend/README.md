# Elysium Web - MeshNet Dashboard

Beautiful web dashboard for MeshNet in Claude Code style.

## Local Development

1. Make sure the Rust node is running with API enabled:
```bash
cd core
cargo run --bin core --release -- 8080
```

2. Start the FastAPI backend:
```bash
cd web/backend
pip install fastapi uvicorn
python app.py
```

3. Open `index.html` in your browser or serve it with a simple HTTP server:
```bash
cd web/frontend
python3 -m http.server 8081
# Then open http://localhost:8081
```

## GitHub Pages Deployment

The frontend is automatically deployed to GitHub Pages when changes are pushed to the `main` branch.

The site will be available at: `https://<username>.github.io/<repo>/`

**Note:** For GitHub Pages, you'll need to configure the API endpoint in `assets/app.js` to point to your backend API, or use a proxy service.

## Configuration

To use with a different API endpoint, modify the `API_BASE` constant in `assets/app.js`:

```javascript
const API_BASE = 'https://your-api-domain.com/api';
```

## Features

- 🎨 Claude Code inspired dark theme
- 📊 Real-time network status
- 👥 Peer list with connection status
- 💬 Mesh chat interface
- 🌐 Mesh sites browser
- 📱 Responsive design






