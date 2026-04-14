#!/usr/bin/env bash
# Build static HTML from ../language-guide.md (not committed under src/).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
SRC="$ROOT/src"
DOCS="$ROOT/.."
REPO_BASE="${RIB_BOOK_REPO_URL:-https://github.com/golemcloud/rib/blob/main}"

mkdir -p "$SRC"
cp "$DOCS/language-guide.md" "$SRC/guide.md"
cp "$DOCS/example.wit" "$SRC/example.wit"

# Relative ../ links work on GitHub; for the static book they must point at the repo.
python3 <<PY
from pathlib import Path
p = Path("${SRC}/guide.md")
t = p.read_text()
t = t.replace("](../README.md", "](${REPO_BASE}/README.md")
t = t.replace("](../rib-lang/README.md", "](${REPO_BASE}/rib-lang/README.md")
p.write_text(t)
PY

mdbook build "$ROOT"
# Serve example.wit next to the generated HTML (guide links to example.wit)
cp "$DOCS/example.wit" "$ROOT/book/example.wit"
