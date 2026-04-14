# Language guide as a static site (mdBook)

The canonical source is **[`../language-guide.md`](../language-guide.md)**. This folder builds it with **[mdBook](https://github.com/rust-lang/mdBook)** so you can host it on **GitHub Pages** or any static file host. **`./build.sh`** generates **`src/example-wit.md`** from **[`../example.wit`](../example.wit)** (second sidebar chapter, highlighted in the browser) and rewrites **`example.wit`** links in the guide to that page in the built HTML.

## Local build

```bash
cargo install mdbook --version 0.5.2   # once; 0.5+ adds in-page headings in the sidebar (matches CI)
cd docs/language-guide-book
chmod +x build.sh
./build.sh
```

Open **`book/index.html`** in a browser (or run `mdbook serve` after `./build.sh`—you may need to re-run `./build.sh` when the guide changes).

Forks: override the base URL used to rewrite `../README.md` / `../rib-lang` links:

```bash
RIB_BOOK_REPO_URL="https://github.com/YOU/rib/blob/main" ./build.sh
```

## After merging to `main` or `master`

1. **Merge** your branch into **`main`** or **`master`** (the workflow listens to both).

2. **Enable GitHub Pages**: **Settings → Pages → Build and deployment → Source: “Deploy from a branch”** → Branch **`gh-pages`** → folder **`/ (root)`**.  
   The workflow **`.github/workflows/language-guide-pages.yml`** pushes built HTML to **`gh-pages`** on each qualifying push (or run it manually via **Actions → Language guide (GitHub Pages) → Run workflow**).

3. **Wait for the first run** to finish, then open the URL GitHub shows under **Settings → Pages** (often **`https://<org>.github.io/<repo>/`** for a project site).

### If you use a custom domain

Add **`[output.html] site-url`** in **`book.toml`** (see [mdBook docs](https://rust-lang.github.io/mdBook/format/configuration/renderers.html)) and configure DNS + **Settings → Pages** as GitHub documents.

### Alternative hosts

Any host that serves static files (Netlify, Cloudflare Pages, S3, etc.) can take the contents of **`docs/language-guide-book/book/`** after `./build.sh`—no GitHub Actions required.
