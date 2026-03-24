#!/usr/bin/env bash
set -euo pipefail

# Build pages/ for GitHub Pages deployment.
# Run this before committing pages/ changes.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TML_DIR="$PROJECT_DIR/../../sw-vibe-coding/tml24c"
TC24R="$PROJECT_DIR/../../sw-vibe-coding/tc24r/components/cli/target/release/tc24r"

# 1. Recompile all REPL variants from tml24c
echo "=== Compiling REPL variants ==="
for v in bare minimal standard full scheme; do
  echo "  repl-$v..."
  "$TC24R" "$TML_DIR/src/repl-$v.c" -I "$TML_DIR/src" -o "$PROJECT_DIR/asm/repl-$v.s"
done

# 2. Build WASM for GitHub Pages (with correct public URL)
echo "=== Building pages/ ==="
cd "$PROJECT_DIR"
trunk build --release --public-url /web-tml24c/ -d pages

# 3. Ensure .nojekyll exists (trunk build -d wipes the output dir)
touch pages/.nojekyll
git add -f pages/.nojekyll 2>/dev/null || true

echo "=== Done ==="
echo "Pages built in: $PROJECT_DIR/pages/"
echo "To preview locally: ./scripts/serve.sh"
echo "To deploy: git add pages/ && git commit && git push"
