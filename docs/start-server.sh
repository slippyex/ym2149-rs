#!/bin/bash

# Simple web server for testing YM2149 WASM player
# Run this script and open http://localhost:8000/simple-player.html

cd "$(dirname "$0")"

echo "Starting YM2149 Web Player server..."
echo ""
echo "Open your browser and go to:"
echo "  http://localhost:8000/simple-player.html"
echo ""
echo "Test files available:"
ls -1 *.ym 2>/dev/null | while read file; do
    echo "  - $file"
done
echo ""
echo "Press Ctrl+C to stop the server"
echo ""

python3 -m http.server 8000 --bind 127.0.0.1
