# CUI Desktop

A cross-platform desktop shell for [CUI](https://github.com/YaoApp/cui) (the web-based admin interface of [Yao App Engine](https://yaoapps.com)). It wraps CUI in a native desktop window with a built-in local proxy, enabling seamless authentication, OAuth login, and same-origin access to all Yao server resources — without modifying CUI or Yao source code.

Developers can rebrand and redistribute it as their own desktop application (e.g. **Yao Agents** at [yaoagents.com](https://yaoagents.com)) via `config.json`.

## Features

- **Cross-platform** — macOS, Windows, Linux (built on Tauri 2.0)
- **Local proxy** — Transparent HTTP proxy forwards all requests to the remote Yao server, ensuring same-origin for CUI, SUI, and all server-rendered pages
- **Cookie management** — Proxy-side cookie jar handles secure cookies on HTTP localhost, bypassing browser HTTPS restrictions
- **OAuth login** — Google/GitHub OAuth works seamlessly via navigation interception and configurable proxy port
- **System tray** — Runs in the background; close window hides to tray, click tray icon to restore
- **Multi-server** — Manage multiple Yao server connections, switch between them
- **Dark mode & i18n** — Light/dark theme and Chinese/English, synced to CUI automatically
- **Drag & drop** — Files and images can be dragged into the app
- **Brandable** — Customize app name, logo, port, theme, update endpoints, and default servers via `config.json`
- **Non-invasive** — Loads CUI build output as-is, no modifications to Yao or CUI required

## Quick Start

### Prerequisites

- Node.js >= 18
- Rust toolchain ([rustup.rs](https://rustup.rs))
- macOS / Windows / Linux

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

## How It Works

1. User selects a Yao server and clicks **Connect**
2. Rust backend starts a local HTTP proxy at `127.0.0.1:<port>` (default `15099`)
3. Proxy serves CUI static assets at `/__yao_admin_root/`
4. All other requests (`/api/*`, `/web/*`, `/v1/*`, SUI pages, etc.) are proxied to the remote server — guaranteeing same-origin
5. Proxy intercepts `Set-Cookie` headers, stores them in a local cookie jar, and injects them on outgoing requests
6. OAuth callbacks are intercepted by Tauri's navigation handler and routed through the proxy
7. CUI runs in the WebView, fully unaware it's behind a proxy

## Developer Config

`config.json` at project root (bundled into the app). Developers can rebrand the application by changing these fields:

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
      "label": "Local Server"
    }
  ]
}
```

| Field | Description |
|---|---|
| `name` | App display name (e.g. "Yao Agents") |
| `logo` | Logo image path (empty = default icon) |
| `port` | Local proxy port — register `http://127.0.0.1:<port>` as OAuth redirect URI |
| `theme.primaryColor` | Primary UI color |
| `updater` | Auto-update configuration |
| `servers` | Pre-configured server list for end users |

## OAuth Setup

Register the following redirect URI in your OAuth provider (Google, GitHub, etc.):

```
http://127.0.0.1:15099/__yao_admin_root/auth/back/google
```

The port must match `config.json` → `port`. Google OAuth allows `http://127.0.0.1` as a valid redirect URI.

## Project Structure

```
cui-desktop/
├── config.json             # Developer config (bundled)
├── src/                    # Frontend (Vite + TypeScript)
│   ├── main.ts             # Entry, routing
│   ├── pages/
│   │   ├── servers.ts      # Server selection page
│   │   ├── settings.ts     # Settings page
│   │   └── app.ts          # Main page (loads CUI via proxy)
│   ├── lib/
│   │   ├── api.ts          # Tauri command bindings
│   │   ├── store.ts        # Local storage (Tauri plugin-store)
│   │   ├── router.ts       # Simple SPA router
│   │   └── i18n.ts         # Internationalization & theme
│   └── styles/
│       └── global.css      # Global styles (light/dark)
├── src-tauri/              # Tauri Rust backend
│   └── src/
│       ├── main.rs         # Entry
│       ├── lib.rs          # App init, tray, navigation interceptor
│       ├── app_conf.rs     # Developer config (config.json)
│       ├── proxy.rs        # Local HTTP proxy server (core)
│       ├── config.rs       # Proxy state & cookie jar
│       └── commands.rs     # Tauri commands
├── scripts/                # Build scripts
├── cui/                    # CUI source (git clone, gitignored)
└── cui-dist/               # CUI build output (gitignored)
```

## Building for Release

```bash
cargo tauri build
```

Output is at `src-tauri/target/release/bundle/`.

## License

MIT
