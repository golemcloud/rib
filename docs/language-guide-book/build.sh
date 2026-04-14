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
# Point example.wit links at an in-book page (syntax-highlighted) instead of raw .wit.
# Quoted heredoc: WIT text and markdown backticks must not be expanded by the shell.
RIB_SRC="$SRC" RIB_DOCS="$DOCS" RIB_REPO_BASE="$REPO_BASE" python3 <<'PY'
import os
from pathlib import Path

repo_base = os.environ["RIB_REPO_BASE"]
src = Path(os.environ["RIB_SRC"])
do_docs = Path(os.environ["RIB_DOCS"])

wit = (do_docs / "example.wit").read_text(encoding="utf-8")
# Concatenate (WIT text contains `{line-item}` etc., so avoid str.format / f-string on `wit`).
wit_page = (
    "# Example WIT (`example.wit`)\n\n"
    "Same file as [docs/example.wit](" + repo_base + "/docs/example.wit) in the repository. "
    "The block is WIT; the book applies TypeScript-style highlighting as a rough approximation.\n\n"
    "```typescript\n" + wit + "\n```\n"
)
(src / "example-wit.md").write_text(wit_page, encoding="utf-8")

p = src / "guide.md"
t = p.read_text(encoding="utf-8")
t = t.replace("](../README.md", "](" + repo_base + "/README.md")
t = t.replace("](../rib-lang/README.md", "](" + repo_base + "/rib-lang/README.md")
t = t.replace("](example.wit)", "](example-wit.html)")
p.write_text(t, encoding="utf-8")
PY

mdbook build "$ROOT"
# Raw copy still available for tools / deep links
cp "$DOCS/example.wit" "$ROOT/book/example.wit"
