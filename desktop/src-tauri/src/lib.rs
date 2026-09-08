//! DeepSeek Harness desktop shell.
//!
//! Spawns the bundled `dsh web` server (the harness' browser surface) as a child
//! process, waits for its readiness URL line, then navigates the native window
//! to that URL. Kills the server when the app exits.
//!
//! The shell also owns a plugin-manager window (menu: 插件 → 插件管理器…): it runs
//! the harness' own `dsh plugin --profile web` command with a bundled pnpm on
//! PATH, so plugins install into the app's web profile without touching the
//! read-only bundled harness.

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
};
use url::Url;

/// The running harness server child process, so it can be killed on exit.
struct ServerChild(Arc<Mutex<Option<Child>>>);

/// The readiness line the shell prints once the web server is listening:
/// `dsh web: http://127.0.0.1:<port>`. Only the label prefix is matched; the
/// `http://…` URL itself is kept whole for the scheme check.
const URL_LINE_PREFIX: &str = "dsh web: ";

/// Pull the harness URL out of a stdout line, if it is the readiness line.
fn harness_url_of(line: &str) -> Option<String> {
    let start = line.find(URL_LINE_PREFIX)?;
    let rest = &line[start + URL_LINE_PREFIX.len()..];
    let url = rest.split_whitespace().next()?.to_string();
    (url.starts_with("http://") || url.starts_with("https://")).then_some(url)
}

/// JavaScript that performs the 0.1.3 browser-session token exchange from the
/// page's own origin. It only acts when the server's "authentication required"
/// page is showing; a page that already loaded the UI is left untouched. The
/// same-origin `fetch` stores the HttpOnly session cookie (the subresource
/// path honors the 303 `Set-Cookie`, which a top-level navigation does not in
/// WKWebView); reloading the clean root then serves the real index.
fn auth_handshake_js(token: &str) -> String {
    format!(
        r#"(function () {{
  var body = (document.body && (document.body.innerText || document.body.textContent)) || '';
  if (body.indexOf('dsh web authentication required') === -1) return;
  fetch('/?token={token}', {{ credentials: 'include', redirect: 'follow' }})
    .catch(function () {{}})
    .then(function () {{ window.location.href = '/'; }});
}})();"#,
        token = token,
    )
}

/// Resource copying can drop the exec bit; make sure the bundled Node binary
/// is executable before spawning it.
#[cfg(unix)]
fn ensure_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.mode() & 0o111 == 0 {
            perms.set_mode(perms.mode() | 0o755);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

/// App-data relative locations the shell owns: the bundled Node binary, the
/// bundled pnpm bin dir, the extracted harness, and the app's DSH_HOME.
fn app_paths(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf), String> {
    let resources = app.path().resource_dir().map_err(|e| e.to_string())?;
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let node_bin = resources.join("staging").join("node").join("bin").join("node");
    let pnpm_dir = resources.join("staging").join("pnpm").join("bin");
    let harness = data.join(format!("harness-{}", app.package_info().version));
    let dsh_home = data.join("dsh");
    Ok((node_bin, pnpm_dir, harness, dsh_home))
}

/// PATH with the bundled pnpm and node bin dirs first, so `dsh plugin` finds
/// `pnpm` and plugin lifecycle scripts run on the bundled Node.
fn prepend_bin_dirs(pnpm_dir: &Path, node_dir: &Path) -> String {
    let mut dirs = vec![pnpm_dir.to_path_buf(), node_dir.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(dirs)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// One `dsh plugin --profile web <args>` invocation (pnpm add/remove/update),
/// capturing the combined output so the plugin-manager window can show it.
#[derive(serde::Serialize)]
struct PluginRunOutput {
    exit_code: i32,
    output: String,
}

#[tauri::command]
async fn plugin_run(app: tauri::AppHandle, args: Vec<String>) -> Result<PluginRunOutput, String> {
    let (node_bin, pnpm_dir, harness, dsh_home) = app_paths(&app)?;
    let path = prepend_bin_dirs(&pnpm_dir, node_bin.parent().expect("node lives in a bin dir"));
    tauri::async_runtime::spawn_blocking(move || {
        let output = Command::new(&node_bin)
            .args(["--import", "tsx/esm"])
            .arg(harness.join("apps").join("cli").join("src").join("bin.ts"))
            .args(["plugin", "--profile", "web"])
            .args(&args)
            .current_dir(&harness)
            .env("DSH_HOME", &dsh_home)
            .env("PATH", &path)
            .env("NO_COLOR", "1")
            .output()
            .map_err(|e| e.to_string())?;
        let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        Ok(PluginRunOutput {
            exit_code: output.status.code().unwrap_or(-1),
            output: combined,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Installed state of the app's web profile: dependency names and the active
/// `dsh.profile.bundles` layer list.
#[derive(serde::Serialize)]
struct PluginStatus {
    profile_dir: String,
    dependencies: Vec<String>,
    bundles: Vec<String>,
}

#[tauri::command]
fn plugin_status(app: tauri::AppHandle) -> Result<PluginStatus, String> {
    let (_, _, _, dsh_home) = app_paths(&app)?;
    let profile_dir = dsh_home.join("profiles").join("web");
    let mut dependencies = Vec::new();
    let mut bundles = Vec::new();
    let manifest_path = profile_dir.join("package.json");
    if let Ok(text) = std::fs::read_to_string(&manifest_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
                dependencies = deps.keys().cloned().collect();
            }
            if let Some(list) = json
                .get("dsh")
                .and_then(|d| d.get("profile"))
                .and_then(|p| p.get("bundles"))
                .and_then(|b| b.as_array())
            {
                bundles = list
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }
        }
    }
    Ok(PluginStatus {
        profile_dir: profile_dir.to_string_lossy().into_owned(),
        dependencies,
        bundles,
    })
}

/// Relaunch the app so a freshly installed plugin's bundle layer loads.
#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe).spawn();
    }
    app.exit(0);
}

/// The built-in Blue Fantasy skin row id (inserted by the web-app bundle patch).
const BLUE_FANTASY_ID: &str = "ui-skin-blue-fantasy";

/// The profile-patch text that disables the Blue Fantasy skin row.
const BLUE_FANTASY_DISABLED: &str = "- id: ui-skin-blue-fantasy\n  disabled: true";

/// Return `existing` with the Blue Fantasy row's `disabled` state set to
/// `enabled`: `enabled` removes any Blue Fantasy row (the skin is on by
/// default), `!enabled` ensures a single disabling row exists. Non-Blue-Fantasy
/// rows and the file's comments are preserved verbatim.
fn theme_patch(existing: &str, enabled: bool) -> String {
    // Drop every Blue Fantasy row block: a top-level `- id:` line plus its
    // indented body (up to the next non-indented line).
    let mut kept: Vec<&str> = Vec::new();
    let mut lines = existing.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim_start().starts_with("- id:") && line.contains(BLUE_FANTASY_ID) {
            while let Some(next) = lines.peek() {
                if next.starts_with(' ') {
                    lines.next();
                } else {
                    break;
                }
            }
            continue;
        }
        kept.push(line);
    }
    if enabled {
        let mut out = kept.join("\n");
        out.push('\n');
        return out;
    }
    // Disabling: ensure a Blue Fantasy disabled row exists. If the file has
    // other rows, append after them; otherwise replace the bare `[]` (the
    // default empty array) with the row.
    if kept.iter().any(|l| l.trim_start().starts_with("- id:")) {
        let mut out = kept.join("\n");
        out.push('\n');
        out.push_str(BLUE_FANTASY_DISABLED);
        out.push('\n');
        out
    } else if let Some(pos) = kept.iter().position(|l| l.trim() == "[]") {
        kept[pos] = BLUE_FANTASY_DISABLED;
        let mut out = kept.join("\n");
        out.push('\n');
        out
    } else {
        let mut out = kept.join("\n");
        out.push('\n');
        out.push_str(BLUE_FANTASY_DISABLED);
        out.push('\n');
        out
    }
}

/// Switch the built-in Blue Fantasy skin on (`enabled`) or off by rewriting
/// the web profile's cordis.patch.yml, then relaunch so the new plugin state
/// loads. The web-UI skin-toggle row drives this command.
#[tauri::command]
fn set_theme(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let (_, _, _, dsh_home) = app_paths(&app)?;
    let profile = dsh_home.join("profiles").join("web");
    std::fs::create_dir_all(&profile).map_err(|e| e.to_string())?;
    let patch = profile.join("cordis.patch.yml");
    let existing = std::fs::read_to_string(&patch).unwrap_or_default();
    std::fs::write(&patch, theme_patch(&existing, enabled)).map_err(|e| e.to_string())?;
    restart_app(app);
    Ok(())
}

/// Show the plugin-manager window, creating it on first use.
fn open_plugin_manager(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("plugin-manager") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    if let Ok(window) = WebviewWindowBuilder::new(
        app,
        "plugin-manager",
        WebviewUrl::App("plugin-manager/index.html".into()),
    )
    .title("插件管理器")
    .inner_size(540.0, 700.0)
    .min_inner_size(440.0, 520.0)
    .build()
    {
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ServerChild(Arc::new(Mutex::new(None))))
        .invoke_handler(tauri::generate_handler![
            plugin_run,
            plugin_status,
            restart_app,
            set_theme
        ])
        .setup(|app| {
            let resources = app
                .path()
                .resource_dir()
                .expect("failed to resolve the bundle Resources directory");
            let node_bin = resources.join("staging").join("node").join("bin").join("node");
            let tarball = resources.join("staging").join("harness.tar.gz");
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve the app data directory");
            let dsh_home = data_dir.join("dsh");
            std::fs::create_dir_all(&dsh_home).expect("failed to create DSH_HOME");

            // The bundled harness ships as one tarball (tar preserves pnpm's
            // node_modules symlink cycles verbatim, unlike a recursive copy).
            // Extract it once into a per-version app-data dir; the marker file
            // makes later launches of the same version skip the extraction. A
            // newer app version names a fresh dir, so updates never run a
            // stale harness; stale copies from older versions are removed.
            let harness_dir_name = format!("harness-{}", app.package_info().version);
            let harness = data_dir.join(&harness_dir_name);
            let extracted_marker = harness.join(".extracted");
            if !extracted_marker.exists() {
                let _ = std::fs::remove_dir_all(&harness);
                std::fs::create_dir_all(&harness).expect("failed to create harness dir");
                let status = Command::new("/usr/bin/tar")
                    .arg("-xzf")
                    .arg(&tarball)
                    .arg("-C")
                    .arg(&harness)
                    .status()
                    .expect("failed to run tar for harness extraction");
                if !status.success() {
                    panic!("harness extraction failed with status {status:?}");
                }
                std::fs::write(&extracted_marker, "ok")
                    .expect("failed to write extraction marker");
            }
            if let Ok(read_dir) = std::fs::read_dir(&data_dir) {
                for entry in read_dir.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with("harness") && name != harness_dir_name {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
            }

            // Fire-and-forget update check: compare the GitHub harness tags with
            // the installed version and surface a once-per-tag notification. It
            // never blocks startup and never downloads anything.
            let check_script = resources.join("resources").join("check-update.sh");
            if check_script.exists() {
                let data_dir = data_dir.clone();
                let _ = Command::new("/bin/bash")
                    .arg(&check_script)
                    .arg(&data_dir)
                    .spawn();
            }

            // Server diagnostics land in the app-data dir so failures are inspectable.
            let boot_log = dsh_home.join("server.log");
            let stderr = std::fs::File::create(&boot_log).expect("failed to create server.log");

            #[cfg(unix)]
            ensure_executable(&node_bin);

            let (url_tx, url_rx) = mpsc::channel::<String>();

            let child_slot = {
                let state = app.state::<ServerChild>();
                state.0.clone()
            };

            // Spawn the server and stream stdout looking for the readiness line.
            {
                let node_bin = node_bin.clone();
                let harness = harness.clone();
                let dsh_home = dsh_home.clone();
                std::thread::spawn(move || {
                    let mut cmd = Command::new(&node_bin);
                    cmd.args([
                        "--import",
                        "tsx/esm",
                        "apps/cli/src/bin.ts",
                        "web",
                        "--no-open",
                        "--port",
                        "0",
                    ])
                    .current_dir(&harness)
                    .env("DSH_HOME", &dsh_home)
                    .env("DSH_DESKTOP", "1")
                    .env("NO_COLOR", "1")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::from(stderr));
                    let child = match cmd.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("[dsh] failed to spawn node: {e}");
                            let _ = url_tx.send(format!("__ERROR__ {e}"));
                            return;
                        }
                    };
                    *child_slot.lock().unwrap() = Some(child);
                    let stdout = child_slot
                        .lock()
                        .unwrap()
                        .as_mut()
                        .expect("child present")
                        .stdout
                        .take()
                        .expect("piped stdout");
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        let line = match line {
                            Ok(l) => l,
                            Err(_) => break,
                        };
                        eprintln!("[dsh] {line}");
                        if let Some(url) = harness_url_of(&line) {
                            let _ = url_tx.send(url);
                            break;
                        }
                    }
                });
            }

            // Wait for readiness on a helper thread; navigate once ready.
            {
                let handle = app.handle().clone();
                let window = app
                    .get_webview_window("main")
                    .expect("the main window is configured");
                std::thread::spawn(move || {
                    match url_rx.recv_timeout(Duration::from_secs(120)) {
                        Ok(url) if !url.starts_with("__ERROR__") => {
                            eprintln!("[dsh] received harness URL: {url}");
                            let url = match Url::parse(&url) {
                                Ok(u) => u,
                                Err(e) => {
                                    eprintln!("[dsh] failed to parse URL {url:?}: {e}");
                                    return;
                                }
                            };
                            let token = url
                                .query_pairs()
                                .find(|(key, _)| key == "token")
                                .map(|(_, value)| value.into_owned())
                                .unwrap_or_default();
                            let win = window.clone();
                            // Owned handle for the auth-handshake thread; the
                            // outer handle stays free for run_on_main_thread.
                            let handshake_handle = handle.clone();
                            let dispatched = handle.run_on_main_thread(move || {
                                eprintln!("[dsh] run_on_main_thread: navigating to {url}");
                                match win.navigate(url.clone()) {
                                    Ok(()) => {
                                        eprintln!("[dsh] navigate() ok");
                                        // New-architecture (0.1.3) browser-session auth:
                                        // WKWebView does not retain the HttpOnly cookie that
                                        // answers the `?token=` 303 of a top-level
                                        // navigation, leaving the window on the server's
                                        // "authentication required" text. Drive the same
                                        // exchange through a same-origin `fetch` (the
                                        // subresource path stores the cookie), reloading the
                                        // clean root once it is held. Repeated evals make the
                                        // handshake robust to load timing.
                                        if !token.is_empty() {
                                            let win = win.clone();
                                            let token = token.clone();
                                            std::thread::spawn(move || {
                                                for delay_ms in [1200u64, 2600, 4200] {
                                                    std::thread::sleep(Duration::from_millis(delay_ms));
                                                    let win = win.clone();
                                                    let token = token.clone();
                                                    let dispatched =
                                                        handshake_handle.run_on_main_thread(move || {
                                                        match win.eval(&auth_handshake_js(&token)) {
                                                            Ok(()) => {
                                                                eprintln!("[dsh] auth handshake eval ok")
                                                            }
                                                            Err(e) => eprintln!(
                                                                "[dsh] auth handshake eval failed: {e}"
                                                            ),
                                                        }
                                                    });
                                                    if dispatched.is_err() {
                                                        eprintln!(
                                                            "[dsh] auth handshake dispatch failed"
                                                        );
                                                        break;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[dsh] navigate() failed: {e}; eval fallback");
                                        let _ = win.eval(&format!(
                                            "window.location.href = '{}';",
                                            url
                                        ));
                                    }
                                }
                                let _ = win.show();
                            });
                            if let Err(e) = dispatched {
                                eprintln!("[dsh] run_on_main_thread dispatch failed: {e}");
                            }
                        }
                        Ok(err) => eprintln!("[dsh] {err}"),
                        Err(_) => eprintln!("[dsh] timed out waiting for the harness server URL"),
                    }
                });
            }

            // App menu: standard Edit (so Cmd+C/V/A work in the webview),
            // View (reload), Window, plus the plugin-manager entry (also ⌘⇧M).
            let handle = app.handle();
            let app_menu = {
                let about = PredefinedMenuItem::about(handle, None, None).expect("about item");
                let sep = PredefinedMenuItem::separator(handle).expect("separator");
                let hide = PredefinedMenuItem::hide(handle, None).expect("hide item");
                let quit = PredefinedMenuItem::quit(handle, None).expect("quit item");
                Submenu::with_items(
                    handle,
                    "DeepSeek Harness",
                    true,
                    &[&about, &sep, &hide, &sep, &quit],
                )
                .expect("app menu")
            };
            let edit_menu = {
                let undo = PredefinedMenuItem::undo(handle, None).expect("undo item");
                let redo = PredefinedMenuItem::redo(handle, None).expect("redo item");
                let sep = PredefinedMenuItem::separator(handle).expect("separator");
                let cut = PredefinedMenuItem::cut(handle, None).expect("cut item");
                let copy = PredefinedMenuItem::copy(handle, None).expect("copy item");
                let paste = PredefinedMenuItem::paste(handle, None).expect("paste item");
                let select_all =
                    PredefinedMenuItem::select_all(handle, None).expect("select all item");
                Submenu::with_items(
                    handle,
                    "编辑",
                    true,
                    &[&undo, &redo, &sep, &cut, &copy, &paste, &select_all],
                )
                .expect("edit menu")
            };
            let view_menu = {
                let reload =
                    MenuItem::with_id(handle, "reload", "重新加载", true, Some("CmdOrCtrl+R"))
                        .expect("reload item");
                Submenu::with_items(handle, "视图", true, &[&reload]).expect("view menu")
            };
            let plugins_menu = {
                let open_plugin = MenuItem::with_id(
                    handle,
                    "open-plugin-manager",
                    "插件管理器…",
                    true,
                    Some("CmdOrCtrl+Shift+M"),
                )
                .expect("plugin-manager menu item");
                Submenu::with_items(handle, "插件", true, &[&open_plugin]).expect("plugins menu")
            };
            let window_menu = {
                let minimize = PredefinedMenuItem::minimize(handle, None).expect("minimize item");
                let close = PredefinedMenuItem::close_window(handle, None).expect("close item");
                Submenu::with_items(handle, "窗口", true, &[&minimize, &close])
                    .expect("window menu")
            };
            let menu = Menu::with_items(
                handle,
                &[&app_menu, &edit_menu, &view_menu, &plugins_menu, &window_menu],
            )
            .expect("menu");
            app.set_menu(menu).expect("set menu");

            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open-plugin-manager" => open_plugin_manager(app),
            "reload" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.reload();
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("failed to build the tauri application")
        .run(|app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
                // Clone the shared slot into an owned Arc, then take the child
                // on its own statement so the MutexGuard temporary is dropped
                // at the semicolon (an if-let scrutinee would keep it alive).
                let slot: Arc<Mutex<Option<Child>>> =
                    app_handle.state::<ServerChild>().0.clone();
                let child = slot.lock().unwrap().take();
                if let Some(mut child) = child {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        });
}
