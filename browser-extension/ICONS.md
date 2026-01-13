# Icons

Placeholder icons are needed for the Chrome extension. You can create simple icons using:

1. **Online tool**: Use https://www.favicon-generator.org/ or similar
2. **Image editor**: Create 16x16, 48x48, and 128x128 PNG files
3. **Command line** (if ImageMagick available):
   ```bash
   convert -size 16x16 xc:'#2563eb' -pointsize 8 -fill white -gravity center -annotate +0+0 'E' icon16.png
   convert -size 48x48 xc:'#2563eb' -pointsize 24 -fill white -gravity center -annotate +0+0 'E' icon48.png
   convert -size 128x128 xc:'#2563eb' -pointsize 64 -fill white -gravity center -annotate +0+0 'E' icon128.png
   ```

For now, the extension will work without icons (Chrome will show a default icon).

