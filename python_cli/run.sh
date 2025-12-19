#!/bin/bash
# Run Python CLI with virtual environment

cd "$(dirname "$0")" || exit 1

# Activate virtual environment if it exists
if [ -d "venv" ]; then
    source venv/bin/activate
fi

# Run CLI
python3 cli.py "$@"

