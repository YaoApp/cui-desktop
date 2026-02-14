# Yao CUI Desktop Client — AI 编码提示词

把这个文件整体作为 prompt 发给 AI 编码工具（Cursor Agent / Codex / Claude Code 等），让它在 `/Users/max/Yao/cui-desktop` 目录下从零搭建项目。

---

## 你是谁

你是一个资深全栈工程师，精通 Tauri 2.0、Rust、TypeScript、Vite、React。你需要在当前目录 `/Users/max/Yao/cui-desktop` 从零搭建一个 Tauri 2.0 桌面客户端项目。

## 项目目标

做一个独立的桌面客户端，用来连接远程 Yao App Engine 服务器。核心思路：

1. **不修改 CUI 源码** — CUI（https://github.com/YaoApp/cui）是 Yao 的 Web 前端，从 GitHub 拉取代码，本地构建，作为静态资源加载。
2. **独立登录/配置** — 客户端自带登录页面和服务器配置页面，不依赖 CUI 的登录流程。
3. **本地代理** — 客户端内置一个本地 HTTP 代理服务器，CUI 的所有 API 请求通过代理转发到远程 Yao 服务器，同时自动注入认证信息。
4. **长效 Token** — 用户登录后获取一个长效 Client Secret（90天有效），存在本地加密存储中，免频繁登录。

## 技术选型

- **桌面框架**: Tauri 2.0（轻量，~10-20MB 安装包）
- **前端**: Vite + TypeScript（原生，不用框架，登录页和设置页很简单）
- **本地代理**: Rust 侧用 `axum` 或 `actix-web` 起一个本地 HTTP Server
- **存储**: Tauri plugin-store（加密存储 Token 和服务器配置）

## CUI 背景知识

CUI 是一个基于 UmiJS Max 4.0.68 + React 18 的 SPA，以下是关键信息：

### 构建方式
```bash
# CUI 仓库: https://github.com/YaoApp/cui
# 主包路径: packages/cui/
# 包管理器: pnpm
# 构建命令:
cd cui && pnpm install && pnpm run build:cui
# 构建产物输出到: packages/cui/dist/
```

### Base Path
CUI 构建时的 base path 是 `/__yao_admin_root/`（由 `.env` 中 `BASE="__yao_admin_root"` 决定）。所有静态资源和路由都在这个前缀下。

### API 发现机制
CUI 启动时会同步请求 `/.well-known/yao` 获取服务器元数据：
```typescript
// GET /.well-known/yao 返回:
{
  "name": "Yao App",
  "version": "0.10.5",
  "openapi": "/v1",          // OpenAPI base URL
  "dashboard": "/admin",      // 管理后台路径
  "issuer_url": "https://..."
}
```
这个请求是**同步 XMLHttpRequest**（在 `services/wellknown.ts` 中）。

### API 请求路径
CUI 的所有 API 请求使用相对路径，需要代理的路径前缀：
- `/v1/*` — OpenAPI 接口（SSE 流式传输，AI 对话用）
- `/api/*` — Legacy API 接口（SSE 流式传输）
- `/.well-known/*` — 服务发现
- `/components/*` — 远程组件
- `/assets/*` — 静态资源
- `/iframe/*` — 内嵌页面
- `/ai/*` — AI 接口
- `/agents/*` — Agent 接口
- `/docs/*` — 文档
- `/tools/*` — 工具
- `/brands/*` — 品牌资源
- `/admin/*` — 管理后台 API

### 认证机制
CUI 使用两套认证体系：

**1. Legacy 模式（旧版，axios）：**
- 登录: `POST /api/__yao/login/admin` → 返回 token
- 请求头: `Authorization: Bearer <token>`

**2. OpenAPI 模式（新版，fetch + Secure Cookie）：**
- 认证入口配置: `GET /v1/user/entry`
- 验证用户: `POST /v1/user/entry/verify` → 返回临时 access_token
- 登录: `POST /v1/user/entry/login` (带 Authorization: Bearer <临时token>)
- 登录成功后服务端设置 `__Secure-access_token` / `__Host-access_token` 等 HttpOnly Cookie
- 后续请求自动携带 Cookie（`credentials: 'include'`）
- CSRF 保护: `X-CSRF-Token` 头

### 关键浏览器 API 使用
- `window.$app` / `window.$global` — 全局状态对象
- `localStorage` / `sessionStorage` — 通过 `@yaoapp/storex` 做持久化（前缀 `xgen`）
- `SystemJS` — 运行时动态模块加载（`runtime.ts`）
- `crypto.subtle` — JWT 签名验证（需要安全上下文）
- `XMLHttpRequest`（同步）— `wellknown.ts` 服务发现
- `postMessage` — iframe 通信

## 项目架构

```
cui-desktop/
├── src/                          # 前端（Vite + TypeScript）
│   ├── main.ts                   # 入口，路由控制
│   ├── pages/
│   │   ├── login.ts              # 登录页（服务器地址 + 用户名密码）
│   │   ├── settings.ts           # 设置页（服务器管理、Token 管理）
│   │   └── app.ts                # 主页（加载 CUI）
│   ├── lib/
│   │   ├── api.ts                # 与 Yao 服务器的 API 通信
│   │   ├── store.ts              # 本地存储（调用 Tauri store）
│   │   └── router.ts             # 简单的 SPA 路由
│   └── styles/
│       └── global.css            # 全局样式
├── src-tauri/                    # Tauri Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs               # 入口
│   │   ├── proxy.rs              # 本地 HTTP 代理服务器（核心）
│   │   ├── config.rs             # 配置管理
│   │   └── commands.rs           # Tauri Commands（前端调后端）
│   └── capabilities/
│       └── default.json
├── scripts/
│   ├── setup.sh                  # 一键初始化（安装依赖、拉 CUI、构建）
│   ├── pull-cui.sh               # 拉取/更新 CUI 代码
│   └── build-cui.sh              # 构建 CUI 静态资源
├── cui/                          # Git submodule → YaoApp/cui（gitignore）
├── cui-dist/                     # CUI 构建产物（gitignore）
├── index.html                    # Vite 入口 HTML
├── package.json
├── vite.config.ts
├── tsconfig.json
└── README.md
```

## 核心实现要点

### 1. 本地代理服务器（proxy.rs）— 最关键的部分

在 Rust 侧启动一个本地 HTTP Server（比如监听 `127.0.0.1:19840`），职责：

**a) 托管 CUI 静态资源：**
- 把 `cui-dist/` 目录下的构建产物挂在 `/__yao_admin_root/` 路径下
- 用户访问 `http://127.0.0.1:19840/__yao_admin_root/` 就能看到 CUI 界面

**b) 代理 API 请求到远程 Yao 服务器：**
- 匹配上面列出的所有 API 路径前缀（`/v1`, `/api`, `/.well-known` 等）
- 转发到用户配置的远程 Yao 服务器地址
- 自动注入认证信息（Token → Cookie 或 Authorization Header）
- **SSE 支持**: `/v1` 和 `/api` 路径要支持 Server-Sent Events 流式传输，禁止响应缓冲
- **WebSocket 支持**: 如果有 WebSocket upgrade 请求也需要转发

**c) 注入认证信息：**
- 从本地存储读取 Token
- 对每个转发请求，自动添加 `Authorization: Bearer <token>` 或设置对应 Cookie
- 处理 CORS（本地代理到远程，Origin 会不同）

### 2. 登录页面（login.ts）

简洁美观的登录页，包含：
- 服务器地址输入框（如 `https://my-yao-server.com`）
- 用户名/邮箱输入框
- 密码输入框
- "记住我" 复选框
- 登录按钮

登录流程：
1. 用户输入服务器地址 → 先请求 `{server}/.well-known/yao` 验证服务器可用
2. 请求 `{server}/v1/user/entry` 获取登录配置
3. 请求 `{server}/v1/user/entry/verify` 验证用户名 → 获取临时 token
4. 请求 `{server}/v1/user/entry/login` 带密码登录 → 获取长效 token
5. 存储 Token + 服务器配置到 Tauri Store（加密）
6. 启动本地代理服务器
7. 在 Tauri 主窗口中加载 `http://127.0.0.1:19840/__yao_admin_root/`

如果服务器不支持 OpenAPI（`/.well-known/yao` 返回 404），回退到 Legacy 模式：
1. 请求 `{server}/api/__yao/login/admin` 带用户名密码登录
2. 获取 token，后续流程相同

### 3. 主窗口加载 CUI（app.ts）

登录成功后：
- 当前 Tauri WebView 直接导航到 `http://127.0.0.1:19840/__yao_admin_root/`
- CUI 的所有请求都走 localhost 代理，CUI 完全无感知
- CUI 请求 `/.well-known/yao` 时，代理从远程服务器获取并返回，CUI 正常初始化

### 4. 设置页面（settings.ts）

- 显示当前连接的服务器信息
- Token 状态（过期时间、手动刷新）
- 服务器列表管理（支持多个服务器配置，切换）
- 登出按钮
- 关于页面（版本信息）

### 5. 构建脚本（scripts/）

**setup.sh — 一键初始化：**
```bash
#!/bin/bash
set -e

echo "=== Yao CUI Desktop Setup ==="

# 1. 检查依赖
command -v node >/dev/null || { echo "需要 Node.js >= 18"; exit 1; }
command -v pnpm >/dev/null || { echo "需要 pnpm"; exit 1; }
command -v cargo >/dev/null || { echo "需要 Rust toolchain"; exit 1; }

# 2. 安装 Tauri CLI
cargo install tauri-cli --version "^2"

# 3. 安装前端依赖
npm install

# 4. 拉取 CUI
bash scripts/pull-cui.sh

# 5. 构建 CUI
bash scripts/build-cui.sh

echo "=== 初始化完成! ==="
echo "运行 'cargo tauri dev' 启动开发模式"
```

**pull-cui.sh — 拉取 CUI：**
```bash
#!/bin/bash
set -e
if [ -d "cui/.git" ]; then
  echo "更新 CUI..."
  cd cui && git pull origin main
else
  echo "克隆 CUI..."
  git clone --depth 1 https://github.com/YaoApp/cui.git cui
fi
```

**build-cui.sh — 构建 CUI：**
```bash
#!/bin/bash
set -e
echo "构建 CUI..."
cd cui && pnpm install && pnpm run build:cui
echo "复制构建产物..."
rm -rf ../cui-dist
cp -r packages/cui/dist ../cui-dist
echo "CUI 构建完成!"
```

## 关键注意事项

1. **SSE 代理不能缓冲** — AI 对话用 SSE 流式传输，代理必须逐字节转发，不能等整个响应完成再返回。设置 `Transfer-Encoding: chunked`，禁用 gzip。

2. **同步 XHR 兼容** — CUI 的 `wellknown.ts` 用同步 `XMLHttpRequest`，这在 Tauri WebView 中可能有问题。代理层要确保 `/.well-known/yao` 响应快速（可以本地缓存）。

3. **crypto.subtle 安全上下文** — JWT 验证需要 `crypto.subtle`，只在 HTTPS 或 localhost 下可用。因为我们用 `http://127.0.0.1` 的本地代理，这没问题。

4. **Cookie 处理** — CUI 使用 `credentials: 'include'`。代理服务器需要正确处理 Set-Cookie 和 Cookie 转发。因为前端和代理都在 localhost，Cookie 域名问题不大。

5. **WebSocket** — CUI 有 VNC 沙盒功能用 WebSocket，代理层需要处理 WebSocket Upgrade。

6. **CUI 构建产物大** — 依赖很多（Monaco Editor、ECharts、AntV 等），构建产物 ~50-80MB。但 Tauri 本身很小，最终安装包预计 ~80-100MB。

7. **macOS 首要** — 先确保 macOS (arm64) 能跑通，后面再做 Windows/Linux。

## 开始

请在当前目录 `/Users/max/Yao/cui-desktop` 初始化整个项目，包括：
1. 用 `cargo tauri init` 初始化 Tauri 项目（或手动创建 `src-tauri/`）
2. 配置 Vite + TypeScript 前端
3. 实现所有源码文件
4. 创建构建脚本
5. 写 README
6. 确保 `cargo tauri dev` 可以启动

开干吧。
