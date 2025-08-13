#!/bin/bash

# Solana RPC Performance Testing - Run All Languages
# Usage: ./run-all.sh [endpoint] [iterations] [format]

ENDPOINT=${1:-"https://api.mainnet-beta.solana.com"}
ITERATIONS=${2:-100}
FORMAT=${3:-"console"}

echo "=== Solana RPC Performance Comparison ==="
echo "Endpoint: $ENDPOINT"
echo "Iterations: $ITERATIONS"
echo "Format: $FORMAT"
echo ""

# Check if advanced benchmark runner should be used
if command -v node >/dev/null 2>&1 && [ -f "package.json" ]; then
    echo "🚀 Using advanced benchmark runner..."
    
    # Install dependencies if needed
    if [ ! -d "node_modules" ]; then
        echo "Installing benchmark dependencies..."
        npm install --silent
    fi
    
    # Run advanced benchmark
    node benchmark.js --endpoint "$ENDPOINT" --iterations "$ITERATIONS" --format "$FORMAT"
    
    # If a report was generated, offer to analyze it
    if [ "$FORMAT" = "json" ] || [ "$FORMAT" = "console" ]; then
        echo ""
        echo "📊 Analysis available - run: node analyze.js"
    fi
    
else
    echo "⚠️  Node.js not found or package.json missing - using basic comparison"
    echo ""

    # Node.js
    echo "🟢 Running Node.js implementation..."
    cd nodejs
    if [ ! -d "node_modules" ]; then
        echo "Installing Node.js dependencies..."
        npm install > /dev/null 2>&1
    fi
    node index.js "$ENDPOINT" "$ITERATIONS"
    cd ..

    echo ""

    # Go
    echo "🔵 Running Go implementation..."
    cd golang
    if [ ! -f "go.sum" ]; then
        echo "Downloading Go dependencies..."
        go mod tidy > /dev/null 2>&1
    fi
    go run main.go "$ENDPOINT" "$ITERATIONS"
    cd ..

    echo ""

    # Rust
    echo "🟠 Running Rust implementation..."
    cd rust
    if [ ! -d "target" ]; then
        echo "Building Rust project..."
        cargo build > /dev/null 2>&1
    fi
    cargo run -- --endpoint "$ENDPOINT" --iterations "$ITERATIONS"
    cd ..

    echo ""
    echo "=== Comparison Complete ==="
fi