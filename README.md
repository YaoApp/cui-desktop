# Yao CUI Desktop

A desktop client for connecting to remote Yao App Engine servers.

## Features

- **Standalone desktop client** — Built on Tauri 2.0, lightweight installer
- **Local proxy** — Built-in HTTP proxy transparently forwards all CUI requests to the remote server
- **Auto authentication** — Proxy-managed cookie jar intercepts and injects auth cookies automatically
- **Non-invasive** — Loads CUI build output as-is, no source modifications
- **SSE support** — AI chat streaming forwarded byte-by-byte, no buffering

## Quick Start

### Prerequisites

- Node.js >= 18
- pnpm (`npm install -g pnpm`)
- Rust toolchain ([rustup.rs](https://rustup.rs))
- macOS (arm64) — other platforms coming later

### One-line Setup

```bash
bash scripts/setup.sh
```

### Manual Setup

```bash
# 1. Install frontend dependencies
npm install

# 2. Pull CUI source
bash scripts/pull-cui.sh

# 3. Build CUI
bash scripts/build-cui.sh

# 4. Start development mode
cargo tauri dev
```

## Project Structure

```
cui-desktop/
├── src/                    # Frontend (Vite + TypeScript)
│   ├── main.ts             # Entry, routing
│   ├── pages/              # Pages
│   │   ├── login.ts        # Connect page
│   │   ├── settings.ts     # Settings page
│   │   └── app.ts          # Main page (loads CUI)
│   ├── lib/                # Utilities
│   │   ├── api.ts          # Tauri command bindings
│   │   ├── store.ts        # Local storage (Tauri plugin-store)
│   │   └── router.ts       # Simple SPA router
│   └── styles/
│       └── global.css      # Global styles
├── src-tauri/              # Tauri Rust backend
│   └── src/
│       ├── main.rs         # Entry
│       ├── lib.rs          # App initialization
│       ├── proxy.rs        # Local HTTP proxy server (core)
│       ├── config.rs       # Config & cookie jar management
│       └── commands.rs     # Tauri commands
├── scripts/                # Build scripts
├── cui/                    # CUI source (git clone, gitignored)
└── cui-dist/               # CUI build output (gitignored)
```

## How It Works

1. User enters a Yao server URL and clicks Connect
2. Rust backend starts a local HTTP proxy at `127.0.0.1:19840`
3. Proxy serves CUI static assets at `/__yao_admin_root/`
4. Proxy forwards API requests to the remote server
5. Proxy intercepts `Set-Cookie` headers and stores them in a local cookie jar
6. On subsequent requests, proxy injects stored cookies — bypassing browser HTTPS cookie restrictions
7. CUI runs in the WebView, fully unaware it's going through a proxy

## Building for Release

```bash
cargo tauri build
```

Output is at `src-tauri/target/release/bundle/`.

## License

MIT
