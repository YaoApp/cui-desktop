mod app_conf;
mod commands;
mod config;
mod proxy;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{
    Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    image::Image,
    WindowEvent,
};
use tauri::webview::{DownloadEvent, NewWindowResponse};
use futures_util::StreamExt;
use tracing::{info, debug, warn};
use tracing_subscriber::EnvFilter;

/// Global counter for generating unique popup window labels
static POPUP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Tracks download destinations set during DownloadEvent::Requested,
/// so we can retrieve the file path in DownloadEvent::Finished
/// (WebKit may return path=None even on success).
static DOWNLOAD_PATHS: std::sync::LazyLock<Mutex<HashMap<String, PathBuf>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

// ========== Download Toast ==========


/// Self-contained JS/CSS for download Toast (bottom-right).
/// Layout inspired by Chrome/VS Code: icon left, text+link right, close top-right.
/// "Show in Folder" is a text link — clean, non-intrusive.
/// All icons are inline SVG. Adapts to light/dark via prefers-color-scheme.
const TOAST_INJECT_JS: &str = r#"
(function(){
var SVG={
  dl:'<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>',
  ok:'<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 11-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>',
  err:'<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>',
  x:'<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>'
};
var S=document.createElement('style');
S.textContent=`
#__yao_dl_toast{position:fixed;bottom:16px;right:16px;z-index:999999;
  display:flex;flex-direction:column;gap:8px;pointer-events:auto;
  font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,"Helvetica Neue",Arial,"PingFang SC","Hiragino Sans GB","Microsoft YaHei",sans-serif}

.__yao_dl_item{position:relative;display:flex;align-items:center;gap:12px;
  border-radius:10px;padding:14px 40px 14px 14px;width:320px;
  backdrop-filter:blur(24px);-webkit-backdrop-filter:blur(24px);
  transition:opacity 0.3s,transform 0.3s;opacity:1;transform:translateX(0)}
.__yao_dl_item.fade-out{opacity:0;transform:translateX(20px)}

.__yao_dl_icon{flex-shrink:0;width:36px;height:36px;border-radius:10px;
  display:flex;align-items:center;justify-content:center}
.__yao_dl_right{flex:1;min-width:0}
.__yao_dl_name{font-size:13px;font-weight:600;line-height:1.4;
  white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.__yao_dl_status{font-size:12px;line-height:1.4;margin-top:2px}
.__yao_dl_bar_wrap{margin-top:6px}
.__yao_dl_bar_bg{height:3px;border-radius:2px;overflow:hidden}
.__yao_dl_bar{height:100%;border-radius:2px;transition:width 0.15s linear}
.__yao_dl_bar.ind{width:30%!important;animation:__yao_dl_mv 1.4s ease-in-out infinite}
.__yao_dl_link{display:inline-flex;align-items:center;gap:3px;
  margin-top:6px;font-size:12px;cursor:pointer;
  border:none;background:none;padding:0;font-family:inherit;line-height:1;
  text-decoration:underline;text-decoration-style:dotted;text-underline-offset:2px}
.__yao_dl_link:hover{text-decoration-style:solid}
.__yao_dl_close{position:absolute;top:10px;right:10px;width:18px;height:18px;
  background:none;border:none;cursor:pointer;padding:0;border-radius:50%;
  display:flex;align-items:center;justify-content:center;transition:background 0.15s}

@media(prefers-color-scheme:dark){
  .__yao_dl_item{background:rgba(28,28,30,0.92);color:#f0f0f0;
    box-shadow:0 8px 30px rgba(0,0,0,0.4),inset 0 0.5px 0 rgba(255,255,255,0.06)}
  .__yao_dl_status{color:rgba(255,255,255,0.5)}
  .__yao_dl_icon{background:rgba(255,255,255,0.07);color:rgba(255,255,255,0.65)}
  .__yao_dl_bar_bg{background:rgba(255,255,255,0.08)}
  .__yao_dl_bar{background:#60a5fa}
  .__yao_dl_ok .__yao_dl_icon{background:rgba(74,222,128,0.12);color:#4ade80}
  .__yao_dl_ok .__yao_dl_status{color:rgba(74,222,128,0.85)}
  .__yao_dl_err .__yao_dl_icon{background:rgba(248,113,113,0.12);color:#f87171}
  .__yao_dl_err .__yao_dl_status{color:rgba(248,113,113,0.85)}
  .__yao_dl_link{color:rgba(255,255,255,0.55)}
  .__yao_dl_link:hover{color:rgba(255,255,255,0.8)}
  .__yao_dl_close{color:rgba(255,255,255,0.3)}
  .__yao_dl_close:hover{background:rgba(255,255,255,0.08);color:rgba(255,255,255,0.6)}
}
@media(prefers-color-scheme:light){
  .__yao_dl_item{background:rgba(255,255,255,0.95);color:#111;
    box-shadow:0 8px 30px rgba(0,0,0,0.08),inset 0 0.5px 0 rgba(255,255,255,1),0 0 0 0.5px rgba(0,0,0,0.04)}
  .__yao_dl_status{color:rgba(0,0,0,0.45)}
  .__yao_dl_icon{background:rgba(0,0,0,0.04);color:rgba(0,0,0,0.5)}
  .__yao_dl_bar_bg{background:rgba(0,0,0,0.06)}
  .__yao_dl_bar{background:#3b82f6}
  .__yao_dl_ok .__yao_dl_icon{background:rgba(22,163,74,0.08);color:#16a34a}
  .__yao_dl_ok .__yao_dl_status{color:#16a34a}
  .__yao_dl_err .__yao_dl_icon{background:rgba(220,38,38,0.06);color:#dc2626}
  .__yao_dl_err .__yao_dl_status{color:#dc2626}
  .__yao_dl_link{color:rgba(0,0,0,0.45)}
  .__yao_dl_link:hover{color:rgba(0,0,0,0.7)}
  .__yao_dl_close{color:rgba(0,0,0,0.2)}
  .__yao_dl_close:hover{background:rgba(0,0,0,0.05);color:rgba(0,0,0,0.5)}
}
@keyframes __yao_dl_mv{0%{transform:translateX(-120%)}100%{transform:translateX(380%)}}
`;
document.head.appendChild(S);
var C=document.createElement('div');C.id='__yao_dl_toast';document.body.appendChild(C);
var items=new Map();
function getEl(id){
  var e=items.get(id);if(e)return e;
  e=document.createElement('div');e.className='__yao_dl_item';
  e.innerHTML='<div class="__yao_dl_icon"></div>'
    +'<div class="__yao_dl_right">'
    +'<div class="__yao_dl_name"></div>'
    +'<div class="__yao_dl_status"></div>'
    +'<div class="__yao_dl_bar_wrap"><div class="__yao_dl_bar_bg"><div class="__yao_dl_bar"></div></div></div>'
    +'<div class="__yao_dl_link_wrap"></div>'
    +'</div>'
    +'<button class="__yao_dl_close">'+SVG.x+'</button>';
  e._hovered=false;
  e.addEventListener('mouseenter',function(){e._hovered=true});
  e.addEventListener('mouseleave',function(){e._hovered=false});
  e.querySelector('.__yao_dl_close').addEventListener('click',function(ev){ev.stopPropagation();removeItem(id)});
  C.appendChild(e);items.set(id,e);
  while(items.size>5){var first=items.keys().next().value;removeItem(first)}
  return e;
}
function autoHide(id,ms){
  var e=items.get(id);if(!e)return;
  clearTimeout(e._timer);
  e._timer=setTimeout(function tick(){
    if(e._hovered){e._timer=setTimeout(tick,1000);return}
    e.classList.add('fade-out');
    setTimeout(function(){removeItem(id)},400);
  },ms);
}
function removeItem(id){var e=items.get(id);if(e){clearTimeout(e._timer);e.remove();items.delete(id)}}
function fmt(b){if(b<1024)return b+'B';if(b<1048576)return(b/1024).toFixed(1)+' KB';return(b/1048576).toFixed(1)+' MB'}
function revealFile(path){
  fetch('/__yao_desktop/reveal',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({path:path})}).catch(function(){});
}
var L={
  zh:{dl:'\u4e0b\u8f7d\u4e2d',show:'\u5728\u6587\u4ef6\u5939\u4e2d\u663e\u793a',
      done:'\u4e0b\u8f7d\u5b8c\u6210',fail:'\u4e0b\u8f7d\u5931\u8d25',unk:'\u672a\u77e5\u9519\u8bef'}
};
function t(k){var o=L[window.__yao_dl_lang];return o&&o[k]||{dl:'Downloading',show:'Show in Folder',done:'Download complete',fail:'Download Failed',unk:'Unknown error'}[k]||k}
window.__yaoDownloadToast={
  start:function(id,filename,total){
    var e=getEl(id);
    e.querySelector('.__yao_dl_icon').innerHTML=SVG.dl;
    e.querySelector('.__yao_dl_name').textContent=filename;
    e.querySelector('.__yao_dl_status').textContent=total>0?t('dl')+' \u00b7 '+fmt(total):t('dl')+'\u2026';
    var bar=e.querySelector('.__yao_dl_bar');
    if(total>0){bar.style.width='0%';bar.classList.remove('ind')}
    else{bar.style.width='';bar.classList.add('ind')}
    e.querySelector('.__yao_dl_bar_wrap').style.display='';
    e.querySelector('.__yao_dl_link_wrap').innerHTML='';
    e.className='__yao_dl_item';
  },
  progress:function(id,loaded,total){
    var e=items.get(id);if(!e)return;
    var pct=total>0?Math.min(100,Math.round(loaded*100/total)):0;
    var bar=e.querySelector('.__yao_dl_bar');
    if(total>0){bar.style.width=pct+'%';bar.classList.remove('ind')}
    e.querySelector('.__yao_dl_status').textContent=fmt(loaded)+(total>0?' / '+fmt(total)+' \u00b7 '+pct+'%':'');
  },
  complete:function(id,filename,filepath){
    var e=items.get(id);if(!e)e=getEl(id);
    e.className='__yao_dl_item __yao_dl_ok';
    e.querySelector('.__yao_dl_icon').innerHTML=SVG.ok;
    e.querySelector('.__yao_dl_name').textContent=filename||t('done');
    e.querySelector('.__yao_dl_status').textContent='';
    e.querySelector('.__yao_dl_bar_wrap').style.display='none';
    var lw=e.querySelector('.__yao_dl_link_wrap');lw.innerHTML='';
    if(filepath){
      var a=document.createElement('button');a.className='__yao_dl_link';
      a.textContent=t('show');
      a.addEventListener('click',function(){revealFile(filepath)});
      lw.appendChild(a);
    }
    autoHide(id,10000);
  },
  fail:function(id,error){
    var e=items.get(id);if(!e)e=getEl(id);
    e.className='__yao_dl_item __yao_dl_err';
    e.querySelector('.__yao_dl_icon').innerHTML=SVG.err;
    e.querySelector('.__yao_dl_name').textContent=t('fail');
    e.querySelector('.__yao_dl_status').textContent=error||t('unk');
    e.querySelector('.__yao_dl_bar_wrap').style.display='none';
    e.querySelector('.__yao_dl_link_wrap').innerHTML='';
    autoHide(id,8000);
  }
};
})()
"#;

fn js_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn toast_eval(action: &str) -> String {
    let lang = config::get_ui_lang();
    format!(
        r#"(function(){{window.__yao_dl_lang="{lang}";if(!window.__yaoDownloadToast){{{inject}}};{action}}})();"#,
        lang = lang,
        inject = TOAST_INJECT_JS,
        action = action,
    )
}

fn eval_on_main(handle: &tauri::AppHandle, js: &str) {
    if let Some(win) = handle.get_webview_window("main") {
        let _ = win.eval(js);
    }
}

fn open_in_system_browser(url: &str) {
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg(url).spawn(); }
    #[cfg(target_os = "windows")]
    { let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url]).spawn(); }
    #[cfg(target_os = "linux")]
    { let _ = std::process::Command::new("xdg-open").arg(url).spawn(); }
}

fn is_external_url(url: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url) {
        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return false;
        }
        let host = parsed.host_str().unwrap_or("");
        if host == "127.0.0.1" || host == "localhost" || host == "::1" {
            return false;
        }
        let state = config::get_proxy_state();
        if !state.server_url.is_empty() {
            if let Ok(server) = url::Url::parse(&state.server_url) {
                if parsed.host() == server.host() {
                    return false;
                }
            }
        }
        return true;
    }
    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("cui_desktop_lib=info"))
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Load developer config.json at startup
            load_app_conf_from_resources(app.handle());

            // Store AppHandle globally so the proxy can call native APIs
            config::set_app_handle(app.handle().clone());

            // ── System Tray ──
            setup_tray(app)?;

            // Channel for navigation redirect requests (main window)
            let (tx, rx) = std::sync::mpsc::channel::<String>();

            // Clone AppHandle for use in closures
            let app_handle = app.handle().clone();
            let app_handle_dl = app.handle().clone();

            // Create the main window manually so we can attach on_navigation + on_new_window
            let window = WebviewWindowBuilder::new(
                    app,
                    "main",
                    WebviewUrl::App("index.html".into()),
                )
                .title("Yao Agents")
                .inner_size(1280.0, 860.0)
                .min_inner_size(900.0, 600.0)
                .center()
                .resizable(true)
                .decorations(true)
                .disable_drag_drop_handler()
                .on_navigation(move |url| {
                    let url_str = url.as_str();

                    // Always allow: tauri://, localhost
                    if url_str.starts_with("tauri://")
                        || url_str.starts_with("http://localhost")
                    {
                        return true;
                    }

                    // If navigating to the remote server, intercept and redirect through proxy
                    let state = config::get_proxy_state();
                    let local_base = format!("http://127.0.0.1:{}", state.port);

                    // Allow our own proxy
                    if url_str.starts_with(&local_base) {
                        return true;
                    }

                    if state.running && !state.server_url.is_empty() {
                        let remote = state.server_url.trim_end_matches('/');
                        if url_str.starts_with(remote) {
                            let proxy_url = url_str.replacen(remote, &local_base, 1);
                            info!("OAuth intercept: {} -> {}", url_str, proxy_url);
                            let _ = tx.send(proxy_url);
                            return false; // Block direct navigation to server
                        }
                    }

                    // Allow all other navigation (Google OAuth, GitHub, etc.)
                    debug!("External navigation: {}", url_str);
                    true
                })
                // Intercept window.open / target="_blank":
                // Return Deny immediately (avoids crash inside WebKit's createNewPage
                // callback), then spawn a new Tauri window asynchronously.
                // Authentication still works because the proxy manages cookies server-side.
                .on_new_window(move |url, _features| {
                    let url_str = url.to_string();
                    let handle = app_handle.clone();
                    info!("New window request: {}", url_str);

                    std::thread::spawn(move || {
                        let state = config::get_proxy_state();
                        let final_url = if state.running && !state.server_url.is_empty() {
                            let remote = state.server_url.trim_end_matches('/');
                            let local_base = format!("http://127.0.0.1:{}", state.port);
                            if url_str.starts_with(remote) {
                                url_str.replacen(remote, &local_base, 1)
                            } else {
                                url_str.clone()
                            }
                        } else {
                            url_str.clone()
                        };

                        if is_file_download_url(&final_url) {
                            spawn_file_download(handle, final_url);
                            return;
                        }

                        if is_external_url(&final_url) {
                            info!("Opening in system browser: {}", final_url);
                            open_in_system_browser(&final_url);
                            return;
                        }

                        let parsed = match url::Url::parse(&final_url) {
                            Ok(u) => u,
                            Err(e) => {
                                warn!("Failed to parse popup URL: {} — {}", final_url, e);
                                return;
                            }
                        };

                        let n = POPUP_COUNTER.fetch_add(1, Ordering::SeqCst);
                        let label = format!("popup_{}", n);
                        info!("Creating popup window: {} -> {}", label, final_url);
                        let handle_dl = handle.clone();
                        let handle_nw = handle.clone();
                        match WebviewWindowBuilder::new(
                            &handle,
                            &label,
                            WebviewUrl::External(parsed),
                        )
                        .title("Yao Agents")
                        .inner_size(1100.0, 780.0)
                        .min_inner_size(600.0, 400.0)
                        .center()
                        .resizable(true)
                        .disable_drag_drop_handler()
                        .on_document_title_changed(|wv, title| {
                            let _ = wv.set_title(&title);
                        })
                        .on_new_window(move |url, _features| {
                            let url_str = url.to_string();
                            let h = handle_nw.clone();
                            info!("Popup new window request: {}", url_str);

                            std::thread::spawn(move || {
                                let state = config::get_proxy_state();
                                let popup_url = if state.running && !state.server_url.is_empty() {
                                    let remote = state.server_url.trim_end_matches('/');
                                    let local_base = format!("http://127.0.0.1:{}", state.port);
                                    if url_str.starts_with(remote) {
                                        url_str.replacen(remote, &local_base, 1)
                                    } else {
                                        url_str.clone()
                                    }
                                } else {
                                    url_str.clone()
                                };

                                if is_file_download_url(&popup_url) {
                                    spawn_file_download(h, popup_url);
                                    return;
                                }

                                if is_external_url(&popup_url) {
                                    info!("Opening in system browser: {}", popup_url);
                                    open_in_system_browser(&popup_url);
                                    return;
                                }

                                let p = match url::Url::parse(&popup_url) {
                                    Ok(u) => u,
                                    Err(_) => return,
                                };
                                let m = POPUP_COUNTER.fetch_add(1, Ordering::SeqCst);
                                let lbl = format!("popup_{}", m);
                                let h_dl2 = h.clone();
                                let _ = WebviewWindowBuilder::new(&h, &lbl, WebviewUrl::External(p))
                                    .title("Yao Agents")
                                    .inner_size(1100.0, 780.0)
                                    .min_inner_size(600.0, 400.0)
                                    .center()
                                    .resizable(true)
                                    .on_download(move |wv, event| {
                                        match event {
                                            DownloadEvent::Requested { url, destination } => {
                                                if let Ok(dl) = h_dl2.path().download_dir() {
                                                    let _ = std::fs::create_dir_all(&dl);
                                                    let f = destination.file_name()
                                                        .map(|f| f.to_string_lossy().to_string())
                                                        .unwrap_or_else(|| "download".to_string());
                                                    *destination = dl.join(&f);
                                                }
                                                if let Ok(mut map) = DOWNLOAD_PATHS.lock() {
                                                    map.insert(url.as_str().to_string(), destination.clone());
                                                }
                                                let fname = destination.file_name()
                                                    .map(|f| f.to_string_lossy().to_string())
                                                    .unwrap_or_else(|| "download".to_string());
                                                info!("Nested popup download: {} -> {:?}", url.as_str(), destination);
                                                let _ = wv.eval(&toast_eval(&format!(
                                                    r#"window.__yaoDownloadToast.start("{}","{}",0)"#,
                                                    js_escape(url.as_str()), js_escape(&fname)
                                                )));
                                            }
                                            DownloadEvent::Finished { url, path, success } => {
                                                let saved = DOWNLOAD_PATHS.lock().ok()
                                                    .and_then(|mut m| m.remove(url.as_str()));
                                                let resolved = path.as_ref().cloned().or(saved);
                                                info!("Nested popup download done: {} success={} path={:?}", url.as_str(), success, resolved);
                                                if success {
                                                    let fname = resolved.as_ref()
                                                        .and_then(|p| p.file_name())
                                                        .map(|f| f.to_string_lossy().to_string())
                                                        .unwrap_or_else(|| "download".to_string());
                                                    let fpath = resolved.as_ref()
                                                        .map(|p| js_escape(&p.to_string_lossy()))
                                                        .unwrap_or_default();
                                                    let _ = wv.eval(&toast_eval(&format!(
                                                        r#"window.__yaoDownloadToast.complete("{}","{}","{}")"#,
                                                        js_escape(url.as_str()), js_escape(&fname), fpath
                                                    )));
                                                } else {
                                                    let _ = wv.eval(&toast_eval(&format!(
                                                        r#"window.__yaoDownloadToast.fail("{}","")"#,
                                                        js_escape(url.as_str())
                                                    )));
                                                }
                                            }
                                            _ => {}
                                        }
                                        true
                                    })
                                    .build();
                            });

                            NewWindowResponse::Deny
                        })
                        .on_download(move |wv, event| {
                            match event {
                                DownloadEvent::Requested { url, destination } => {
                                    if let Ok(dl_dir) = handle_dl.path().download_dir() {
                                        let _ = std::fs::create_dir_all(&dl_dir);
                                        let fname = destination.file_name()
                                            .map(|f| f.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "download".to_string());
                                        *destination = dl_dir.join(&fname);
                                    }
                                    if let Ok(mut map) = DOWNLOAD_PATHS.lock() {
                                        map.insert(url.as_str().to_string(), destination.clone());
                                    }
                                    let fname = destination.file_name()
                                        .map(|f| f.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "download".to_string());
                                    info!("Popup download: {} -> {:?}", url.as_str(), destination);
                                    let _ = wv.eval(&toast_eval(&format!(
                                        r#"window.__yaoDownloadToast.start("{}","{}",0)"#,
                                        js_escape(url.as_str()), js_escape(&fname)
                                    )));
                                }
                                DownloadEvent::Finished { url, path, success } => {
                                    let saved = DOWNLOAD_PATHS.lock().ok()
                                        .and_then(|mut m| m.remove(url.as_str()));
                                    let resolved = path.as_ref().cloned().or(saved);
                                    info!("Popup download done: {} success={} path={:?}", url.as_str(), success, resolved);
                                    if success {
                                        let fname = resolved.as_ref()
                                            .and_then(|p| p.file_name())
                                            .map(|f| f.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "download".to_string());
                                        let fpath = resolved.as_ref()
                                            .map(|p| js_escape(&p.to_string_lossy()))
                                            .unwrap_or_default();
                                        let _ = wv.eval(&toast_eval(&format!(
                                            r#"window.__yaoDownloadToast.complete("{}","{}","{}")"#,
                                            js_escape(url.as_str()), js_escape(&fname), fpath
                                        )));
                                    } else {
                                        let _ = wv.eval(&toast_eval(&format!(
                                            r#"window.__yaoDownloadToast.fail("{}","")"#,
                                            js_escape(url.as_str())
                                        )));
                                    }
                                }
                                _ => {}
                            }
                            true
                        })
                        .build()
                        {
                            Ok(_) => info!("Popup window created: {}", label),
                            Err(e) => warn!("Failed to create popup window: {}", e),
                        }
                    });

                    NewWindowResponse::Deny
                })
                .on_download(move |webview, event| {
                    match event {
                        DownloadEvent::Requested { url, destination } => {
                            if let Ok(download_dir) = app_handle_dl.path().download_dir() {
                                let _ = std::fs::create_dir_all(&download_dir);
                                let filename = destination.file_name()
                                    .map(|f| f.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "download".to_string());
                                *destination = download_dir.join(&filename);
                            }
                            if let Ok(mut map) = DOWNLOAD_PATHS.lock() {
                                map.insert(url.as_str().to_string(), destination.clone());
                            }
                            let fname = destination.file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_else(|| "download".to_string());
                            info!("Download started: {} -> {:?}", url.as_str(), destination);
                            let _ = webview.eval(&toast_eval(&format!(
                                r#"window.__yaoDownloadToast.start("{}","{}",0)"#,
                                js_escape(url.as_str()), js_escape(&fname)
                            )));
                        }
                        DownloadEvent::Finished { url, path, success } => {
                            let saved = DOWNLOAD_PATHS.lock().ok()
                                .and_then(|mut m| m.remove(url.as_str()));
                            let resolved = path.as_ref().cloned().or(saved);
                            if success {
                                let fname = resolved.as_ref()
                                    .and_then(|p| p.file_name())
                                    .map(|f| f.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "download".to_string());
                                let fpath = resolved.as_ref()
                                    .map(|p| js_escape(&p.to_string_lossy()))
                                    .unwrap_or_default();
                                info!("Download complete: {} -> {:?}", url.as_str(), resolved);
                                let _ = webview.eval(&toast_eval(&format!(
                                    r#"window.__yaoDownloadToast.complete("{}","{}","{}")"#,
                                    js_escape(url.as_str()), js_escape(&fname), fpath
                                )));
                            } else {
                                warn!("Download failed: {}", url.as_str());
                                let _ = webview.eval(&toast_eval(&format!(
                                    r#"window.__yaoDownloadToast.fail("{}","")"#,
                                    js_escape(url.as_str())
                                )));
                            }
                        }
                        _ => {}
                    }
                    true
                })
                .build()?;

            // Background thread: process redirect requests
            let webview = window.clone();
            std::thread::spawn(move || {
                while let Ok(url) = rx.recv() {
                    if let Ok(parsed) = url::Url::parse(&url) {
                        info!("Redirecting to proxy: {}", parsed);
                        let _ = webview.navigate(parsed);
                    }
                }
            });

            Ok(())
        })
        // Intercept main window close: hide to tray instead of quitting.
        // Popup windows close normally.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    // Hide window instead of closing
                    let _ = window.hide();
                    api.prevent_close();
                    info!("Main window hidden to tray");
                }
                // Popup windows close normally (no prevent_close)
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_conf,
            commands::check_server,
            commands::start_proxy,
            commands::get_proxy_status,
            commands::update_proxy_token,
            commands::clear_cookies,
            commands::set_preference_cookies,
            commands::set_window_theme,
            commands::set_ui_language,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start Tauri application");
}

/// Build the tray menu with localized labels
fn build_tray_menu<R: tauri::Runtime>(app: &impl Manager<R>) -> Result<Menu<R>, Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", config::tray_label("show"), true, None::<&str>)?;
    let servers = MenuItem::with_id(app, "servers", config::tray_label("servers"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", config::tray_label("quit"), true, None::<&str>)?;
    Ok(Menu::with_items(app, &[&show, &servers, &quit])?)
}

/// When the window is restored from tray, check if it's showing a stale proxy page.
/// If the proxy isn't running but the webview URL points to it, navigate back to the shell UI.
fn restore_if_stale(win: &tauri::WebviewWindow) {
    let url = win.url();
    if let Ok(u) = url {
        let url_str = u.as_str();
        // If on proxy URL but proxy is not running → go back to shell
        if url_str.starts_with("http://127.0.0.1") {
            let state = config::get_proxy_state();
            if !state.running {
                info!("Proxy not running, navigating back to shell UI");
                let _ = win.navigate("tauri://localhost".parse().unwrap());
            }
        }
    }
}

/// Rebuild the tray menu (called when language changes)
pub fn rebuild_tray(app: &tauri::AppHandle) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Ok(menu) = build_tray_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Set up the system tray icon and menu
fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_tray_menu(app)?;

    // Load the tray icon: monochrome template on macOS, colored on Windows/Linux
    let icon = load_tray_icon(app);

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Yao Agents")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        restore_if_stale(&win);
                    }
                }
                "servers" => {
                    let handle = app.clone();
                    if let Some(win) = handle.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let state = config::get_proxy_state();
                        if state.running {
                            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
                            let msg = config::tray_label("switch_confirm");
                            let title = config::tray_label("servers");
                            handle.dialog()
                                .message(msg)
                                .title(title)
                                .buttons(MessageDialogButtons::OkCancel)
                                .show(move |confirmed| {
                                    if confirmed {
                                        if let Some(w) = handle.get_webview_window("main") {
                                            let _ = w.navigate("tauri://localhost".parse().unwrap());
                                        }
                                    }
                                });
                        } else {
                            let _ = win.navigate("tauri://localhost".parse().unwrap());
                        }
                    }
                }
                "quit" => {
                    info!("Quit from tray");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event {
                if let Some(win) = tray.app_handle().get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                    restore_if_stale(&win);
                }
            }
        })
        .build(app)?;

    info!("System tray initialized");
    Ok(())
}

/// Load the tray icon PNG, trying multiple paths (bundled resources, dev icons/).
/// macOS: monochrome template icons; Windows/Linux: colored icons.
fn load_tray_icon(app: &tauri::App) -> Image<'static> {
    // Choose icon set based on platform:
    //   macOS: monochrome template (system auto-inverts for dark/light menu bar)
    //   Windows/Linux: colored icon for better visual identification
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["tray-icon@2x.png", "tray-icon.png"]
    } else {
        &["tray-icon-color@2x.png", "tray-icon-color.png", "tray-icon@2x.png", "tray-icon.png"]
    };

    // 1. Try from bundled resource directory
    if let Ok(resource_dir) = app.handle().path().resource_dir() {
        for name in candidates {
            let path = resource_dir.join("icons").join(name);
            if let Ok(img) = Image::from_path(&path) {
                info!("Tray icon loaded from: {:?}", path);
                return img;
            }
        }
    }

    // 2. Try from project icons/ directory (dev mode)
    for name in candidates {
        let path = std::path::PathBuf::from("icons").join(name);
        if let Ok(img) = Image::from_path(&path) {
            info!("Tray icon loaded from: {:?}", path);
            return img;
        }
    }

    // 3. Fallback: use embedded icon
    warn!("Tray icon not found, using fallback");
    if cfg!(target_os = "macos") {
        Image::from_bytes(include_bytes!("../icons/tray-icon.png")).unwrap()
    } else {
        Image::from_bytes(include_bytes!("../icons/tray-icon-color.png")).unwrap()
    }
}

// ========== File Download Helpers ==========

/// Check if a URL looks like a file download (Yao file API)
fn is_file_download_url(url: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path();
        // Yao file API: /v1/file/{namespace}/{hash}/content
        return path.starts_with("/v1/file/");
    }
    false
}

/// Spawn an async task to download a file from the proxy and save to Downloads folder.
/// Uses streaming to report progress via Toast UI injected into the main window.
fn spawn_file_download(handle: tauri::AppHandle, url: String) {
    info!("File download: {}", url);
    let url_id = js_escape(&url);
    tauri::async_runtime::spawn(async move {
        let download_dir = match handle.path().download_dir() {
            Ok(d) => d,
            Err(e) => {
                warn!("Cannot resolve Downloads directory: {}", e);
                eval_on_main(&handle, &toast_eval(&format!(
                    r#"window.__yaoDownloadToast.fail("{}","Cannot resolve Downloads directory")"#,
                    url_id
                )));
                return;
            }
        };
        if let Err(e) = std::fs::create_dir_all(&download_dir) {
            warn!("Cannot create Downloads directory: {:?} — {}", download_dir, e);
            eval_on_main(&handle, &toast_eval(&format!(
                r#"window.__yaoDownloadToast.fail("{}","Cannot create Downloads directory")"#,
                url_id
            )));
            return;
        }

        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .no_proxy()
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!("Download client error: {}", e);
                eval_on_main(&handle, &toast_eval(&format!(
                    r#"window.__yaoDownloadToast.fail("{}","Client error")"#, url_id
                )));
                return;
            }
        };

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!("Download request failed: {} — {}", url, e);
                eval_on_main(&handle, &toast_eval(&format!(
                    r#"window.__yaoDownloadToast.fail("{}","Request failed")"#, url_id
                )));
                return;
            }
        };

        if !resp.status().is_success() {
            warn!("Download HTTP {}: {}", resp.status(), url);
            eval_on_main(&handle, &toast_eval(&format!(
                r#"window.__yaoDownloadToast.fail("{}","HTTP {}")"#, url_id, resp.status().as_u16()
            )));
            return;
        }

        let filename = extract_download_filename(&resp, &url);
        let dest = ensure_unique_path(download_dir.join(&filename));
        let total = resp.content_length().unwrap_or(0);
        let fname_escaped = js_escape(&filename);

        eval_on_main(&handle, &toast_eval(&format!(
            r#"window.__yaoDownloadToast.start("{}","{}",{})"#,
            url_id, fname_escaped, total
        )));

        let mut stream = resp.bytes_stream();
        let mut buffer = Vec::with_capacity(if total > 0 { total as usize } else { 64 * 1024 });
        let mut downloaded: u64 = 0;
        let mut last_notified: u64 = 0;
        let mut last_time = std::time::Instant::now();
        let mut error: Option<String> = None;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    downloaded += chunk.len() as u64;
                    buffer.extend_from_slice(&chunk);
                    if downloaded - last_notified >= 200_000
                        || last_time.elapsed().as_millis() >= 200
                    {
                        eval_on_main(&handle, &toast_eval(&format!(
                            r#"window.__yaoDownloadToast.progress("{}",{},{})"#,
                            url_id, downloaded, total
                        )));
                        last_notified = downloaded;
                        last_time = std::time::Instant::now();
                    }
                }
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }

        if let Some(err) = error {
            warn!("Download stream error: {} — {}", url, err);
            eval_on_main(&handle, &toast_eval(&format!(
                r#"window.__yaoDownloadToast.fail("{}","Stream error")"#, url_id
            )));
            return;
        }

        if let Err(e) = std::fs::write(&dest, &buffer) {
            warn!("Failed to save file: {:?} — {}", dest, e);
            eval_on_main(&handle, &toast_eval(&format!(
                r#"window.__yaoDownloadToast.fail("{}","Save failed")"#, url_id
            )));
            return;
        }

        info!("Downloaded {} bytes → {:?}", buffer.len(), dest);
        let dest_escaped = js_escape(&dest.to_string_lossy());
        eval_on_main(&handle, &toast_eval(&format!(
            r#"window.__yaoDownloadToast.complete("{}","{}","{}")"#,
            url_id, fname_escaped, dest_escaped
        )));
    });
}

/// Extract a filename from the response Content-Disposition header, falling back to the URL path.
fn extract_download_filename(resp: &reqwest::Response, url: &str) -> String {
    if let Some(cd) = resp.headers().get("content-disposition") {
        // Use from_utf8_lossy instead of to_str() — many servers send raw UTF-8
        // filenames (e.g. Chinese) which are not valid ASCII.
        // to_str() would fail silently and we'd lose the filename.
        let cd_str = String::from_utf8_lossy(cd.as_bytes());
        debug!("Content-Disposition: {}", cd_str);

        let cd_lower = cd_str.to_lowercase();

        // Try: filename*=UTF-8''encoded_name (RFC 5987)
        if let Some(pos) = cd_lower.find("filename*=") {
            let after = &cd_str[pos + 10..];
            // Strip charset prefix like "UTF-8''" or "utf-8''"
            let encoded = if let Some(idx) = after.find("''") {
                &after[idx + 2..]
            } else {
                after
            };
            let end = encoded.find(';').unwrap_or(encoded.len());
            let decoded = percent_decode(encoded[..end].trim());
            let decoded = decoded.trim().trim_matches('"');
            if !decoded.is_empty() {
                info!("Filename from Content-Disposition (RFC5987): {}", decoded);
                return sanitize_filename(decoded);
            }
        }

        // Try: filename="name" or filename=name
        if let Some(pos) = cd_lower.find("filename=") {
            let after = &cd_str[pos + 9..];
            let name = if after.starts_with('"') {
                after[1..].split('"').next().unwrap_or("")
            } else {
                let end = after.find(';').unwrap_or(after.len());
                after[..end].trim()
            };
            if !name.is_empty() {
                info!("Filename from Content-Disposition: {}", name);
                return sanitize_filename(name);
            }
        }
    }

    // Fallback: last meaningful path segment from URL
    if let Ok(parsed) = url::Url::parse(url) {
        let segments: Vec<&str> = parsed.path().split('/').filter(|s| !s.is_empty()).collect();
        for &seg in segments.iter().rev() {
            if seg != "content" && seg != "download" {
                info!("Filename from URL fallback: {}", seg);
                return sanitize_filename(seg);
            }
        }
    }
    "download".to_string()
}

/// Simple percent-decoding (for Content-Disposition filenames)
fn percent_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Remove characters that are illegal in filenames
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect()
}

/// If the path already exists, append (1), (2), … until unique
fn ensure_unique_path(path: std::path::PathBuf) -> std::path::PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = path.extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    for i in 1..1000 {
        let candidate = parent.join(format!("{} ({}){}", stem, i, ext));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

/// Load config.json from bundled resources or project root (dev mode)
fn load_app_conf_from_resources(app: &tauri::AppHandle) {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let path = resource_dir.join("config.json");
        if path.exists() {
            info!("Loading config from resource dir: {:?}", path);
            app_conf::load_app_conf(&resource_dir);
            return;
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("config.json").exists() {
            app_conf::load_app_conf(&cwd);
            return;
        }
        if let Some(parent) = cwd.parent() {
            if parent.join("config.json").exists() {
                app_conf::load_app_conf(&parent.to_path_buf());
                return;
            }
        }
    }

    info!("config.json not found, using defaults");
}
