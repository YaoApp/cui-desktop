# Yao Agents

A desktop client for connecting to remote Yao App Engine servers.

## Features

- **Standalone desktop client** — Built on Tauri 2.0, lightweight installer
- **Local proxy** — Built-in HTTP proxy transparently forwards all requests to the remote server
- **Auto authentication** — Proxy-managed cookie jar intercepts and injects auth cookies automatically
- **OAuth support** — Google/GitHub OAuth works through the proxy with configurable port
- **Non-invasive** — Loads CUI build output as-is, no source modifications
- **SSE support** — AI chat streaming forwarded byte-by-byte, no buffering
- **Developer config** — Brandable via `config.json` (name, logo, port, theme, default servers)

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

## Developer Config

`config.json` at project root (bundled into the app):

```json
{
  "name": "Yao Agents",
  "logo": "",
  "port": 15099,
  "theme": {
    "primaryColor": "#3b82f6"
  },
  "updater": {
    "active": false,
    "endpoints": [],
    "pubkey": ""
  },
  "servers": [
    {
      "url": "http://127.0.0.1:5099",
      "label": "Local Dev Server"
    }
  ]
}
```

| Field | Description |
|---|---|
| `name` | App display name |
| `logo` | Logo image path (empty = text only) |
| `port` | Local proxy port (default `15099`) |
| `theme.primaryColor` | Primary UI color |
| `updater` | Auto-update configuration |
| `servers` | Pre-configured server list for end users |

## Project Structure

```
cui-desktop/
├── config.json             # Developer config (bundled)
├── src/                    # Frontend (Vite + TypeScript)
│   ├── main.ts             # Entry, routing
│   ├── pages/
│   │   ├── servers.ts      # Server selection page
│   │   ├── settings.ts     # Settings page
│   │   └── app.ts          # Main page (loads CUI)
│   ├── lib/
│   │   ├── api.ts          # Tauri command bindings
│   │   ├── store.ts        # Local storage (Tauri plugin-store)
│   │   └── router.ts       # Simple SPA router
│   └── styles/
│       └── global.css      # Global styles
├── src-tauri/              # Tauri Rust backend
│   └── src/
│       ├── main.rs         # Entry
│       ├── lib.rs          # App init, navigation interceptor
│       ├── app_conf.rs     # Developer config (config.json)
│       ├── proxy.rs        # Local HTTP proxy server (core)
│       ├── config.rs       # Proxy state & cookie jar
│       └── commands.rs     # Tauri commands
├── scripts/                # Build scripts
├── cui/                    # CUI source (git clone, gitignored)
└── cui-dist/               # CUI build output (gitignored)
```

## How It Works

1. User selects a Yao server from the server list and clicks Connect
2. Rust backend starts a local HTTP proxy at `127.0.0.1:<port>` (default `15099`)
3. Proxy serves CUI static assets at `/__yao_admin_root/`
4. All other requests are proxied to the remote server (same-origin guarantee)
5. Proxy intercepts `Set-Cookie` headers and stores them in a local cookie jar
6. On subsequent requests, proxy injects stored cookies — bypassing browser HTTPS cookie restrictions
7. OAuth callbacks are intercepted by Tauri's navigation handler and routed through the proxy
8. CUI runs in the WebView, fully unaware it's going through a proxy

## OAuth Setup

Register the following redirect URI in your OAuth provider (Google, GitHub, etc.):

```
http://127.0.0.1:15099/__yao_admin_root/auth/back/google
```

The port must match `config.json` → `port`. Google OAuth allows `http://127.0.0.1` as a valid redirect URI.

## Building for Release

```bash
cargo tauri build
```

Output is at `src-tauri/target/release/bundle/`.

## License

MIT
