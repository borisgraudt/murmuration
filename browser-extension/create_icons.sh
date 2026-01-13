#!/bin/bash
# Create simple colored square icons using sips (macOS built-in)

for size in 16 48 128; do
  # Create a blue square
  sips -s format png -z $size $size --setProperty format png /System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/GenericDocumentIcon.icns --out icon${size}.png 2>/dev/null || \
  # Fallback: create using Python if sips doesn't work
  python3 -c "
from PIL import Image, ImageDraw
img = Image.new('RGB', ($size, $size), color=(37, 99, 235))
draw = ImageDraw.Draw(img)
draw.text(($size//4, $size//4), 'E', fill='white')
img.save('icon${size}.png')
"
done
