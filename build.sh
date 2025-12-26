#!/bin/bash
set -e

PLUGIN_NAME="PlayMix.sdPlugin"
BINARY_NAME="playmix"
DIST_DIR="dist"
PLUGIN_DIR="$DIST_DIR/$PLUGIN_NAME"

echo "Building release binary..."
cargo build --release

echo "Creating plugin directory structure..."
rm -rf "$PLUGIN_DIR"
mkdir -p "$PLUGIN_DIR"

echo "Copying assets..."
cp -r assets/* "$PLUGIN_DIR/"

echo "Copying binary..."
cp "target/release/$BINARY_NAME" "$PLUGIN_DIR/$BINARY_NAME-x86_64-unknown-linux-gnu"

echo "Creating plugin archive..."
cd "$DIST_DIR"
rm -f "$PLUGIN_NAME.zip"
zip -r "$PLUGIN_NAME.zip" "$PLUGIN_NAME"
cd ..

echo "✓ Plugin built successfully!"
echo "  Output: $DIST_DIR/$PLUGIN_NAME.zip"
