#!/bin/bash
# Build script for Rust Python extension using maturin

set -e

echo "🔨 Building Rust Python extension..."

# Check if maturin is installed
if ! command -v maturin &> /dev/null; then
    echo "❌ maturin not found. Installing..."
    pip install maturin
fi

# Build and install in development mode
echo "📦 Building with maturin..."
maturin develop

echo "✅ Build complete! The module should now be importable as 'data_modelling_api'"

# Test import
echo "🧪 Testing import..."
python3 -c "import data_modelling_api; print('✓ Successfully imported data_modelling_api')" || {
    echo "❌ Import failed. Make sure you're in the correct Python environment."
    exit 1
}

echo "✅ All checks passed!"
