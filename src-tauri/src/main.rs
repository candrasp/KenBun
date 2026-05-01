// Mencegah window console terbuka di Windows saat mode release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

// Flag Windows untuk menyembunyikan jendela console (CREATE_NO_WINDOW)
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ── State: Simpan Child process, Log History & Project Path ──────────────────
struct AppState {
    child: Mutex<Option<Child>>,
    log_history: Mutex<Vec<String>>,
    current_path: Mutex<std::path::PathBuf>,
}

// Fungsi untuk membersihkan port tertentu secara paksa di Windows
fn cleanup_port(port: u16) {
    #[cfg(windows)]
    {
        // Gunakan PowerShell untuk mematikan proses di port tertentu, lebih stabil daripada FOR loop CMD
        let script = format!(
            "Get-NetTCPConnection -LocalPort {} -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess | ForEach-Object {{ Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }}",
            port
        );
        let _ = std::process::Command::new("powershell")
            .args(["-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
}

// Fungsi pembantu untuk mematikan proses Bun secara bersih
fn kill_bun_process(mut child: Child, port: u16) {
    let pid = child.id();
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", &format!("taskkill /F /T /PID {}", pid)])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        cleanup_port(port);
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[tauri::command]
fn verify_project(project_path: String, script_path: String) -> Result<(), String> {
    println!("[KenBun] Verifying project at: {}", project_path);
    let clean_path = project_path.replace("\\\\?\\", "");
    let path = std::path::Path::new(&clean_path);
    
    if !path.exists() {
        println!("[KenBun] Verification failed: Path does not exist");
        return Err("PATH_NOT_FOUND".into());
    }

    // 1. Cek lockfiles
    let has_bun_lock = path.join("bun.lockb").exists() || path.join("bun.lock").exists();
    let has_package = path.join("package.json").exists();
    let has_node_lock = path.join("package-lock.json").exists() || path.join("yarn.lock").exists() || path.join("pnpm-lock.yaml").exists();
    
    println!("[KenBun] bun_lock: {}, package: {}, node_lock: {}", has_bun_lock, has_package, has_node_lock);

    // Proteksi: Jika folder Node.js murni (ada lock node/yarn/pnpm tapi tidak ada lock bun)
    if has_node_lock && !has_bun_lock {
        return Err("NODE_PROJECT_DETECTED".into());
    }

    if !has_bun_lock && !has_package {
        return Err("FOLDER_NOT_PROJECT".into());
    }

    // 2. Deteksi framework Node.js dari package.json
    // Framework ini menggunakan node:http/net, bukan Bun.serve, sehingga Preload Interceptor tidak bekerja
    if has_package {
        let pkg_path = path.join("package.json");
        if let Ok(content) = std::fs::read_to_string(&pkg_path) {
            let node_frameworks = [
                ("express", "Express.js"),
                ("fastify", "Fastify"),
                ("koa", "Koa"),
                ("@nestjs/core", "NestJS"),
                ("restify", "Restify"),
                ("hapi", "Hapi.js"),
                ("@hapi/hapi", "Hapi.js"),
                ("feathers", "Feathers.js"),
                ("@feathersjs/feathers", "Feathers.js"),
                ("sails", "Sails.js"),
                ("loopback", "LoopBack"),
                ("@loopback/core", "LoopBack"),
                ("polka", "Polka"),
                ("@adonisjs/core", "AdonisJS"),
                ("total.js", "Total.js"),
                ("derby", "Derby.js"),
                ("meteor-node-stubs", "Meteor.js"),
                ("socket.io", "Socket.io"),
                ("actionhero", "ActionHero"),
                ("@tsed/core", "Ts.ED"),
            ];
            
            let mut detected: Vec<&str> = Vec::new();
            for (pkg_name, display_name) in &node_frameworks {
                // Cari di dependencies maupun devDependencies
                if content.contains(&format!("\"{}\"", pkg_name)) {
                    detected.push(display_name);
                }
            }

            if !detected.is_empty() {
                let frameworks = detected.join(",");
                
                // Cerdas: Baca isi script target. Jika sudah mengandung process.env.PORT atau Bun.env.PORT, izinkan lewat.
                let script_name = if script_path.is_empty() { "index.js" } else { &script_path };
                let target_script = path.join(script_name);
                
                if let Ok(script_content) = std::fs::read_to_string(&target_script) {
                    if !script_content.contains("process.env.PORT") && !script_content.contains("Bun.env.PORT") && !script_content.contains("env.PORT") {
                        println!("[KenBun] Node.js framework(s) detected without env.PORT: {}", frameworks);
                        return Err(format!("NODE_FRAMEWORK_DETECTED:{}", frameworks));
                    } else {
                        println!("[KenBun] Node framework {} detected, but script already uses env.PORT. Allowed.", frameworks);
                    }
                } else {
                    // Jika script tidak bisa dibaca, asumsikan belum diupdate
                    return Err(format!("NODE_FRAMEWORK_DETECTED:{}", frameworks));
                }
            }
        }
    }
    
    // 3. Cek Entry Script
    let script = if script_path.is_empty() { "index.js" } else { &script_path };
    let script_full_path = path.join(script);
    println!("[KenBun] Checking script: {:?}", script_full_path);

    if !script_full_path.exists() {
        return Err(format!("SCRIPT_MISSING:{}", script));
    }
    
    println!("[KenBun] Verification successful");
    Ok(())
}

// Fungsi untuk mengecek apakah port sedang digunakan di Windows
fn is_port_in_use(port: u16) -> bool {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("powershell")
            .args(["-Command", &format!("Get-NetTCPConnection -LocalPort {} -State Listen -ErrorAction SilentlyContinue", port)])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();
        
        if let Ok(out) = output {
            return !out.stdout.is_empty();
        }
    }
    false
}

// ── Command: Jalankan bun index.js ──────────────────────────────────────────
#[tauri::command]
fn start_bun(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    port: u16,
    script_path: String,
    conflict_solver: bool,
    force_port: bool,
) -> Result<u32, String> {
    // Jika Port Conflict Detection aktif, cek dulu
    if conflict_solver {
        if is_port_in_use(port) {
            println!("[KenBun] Port {} is already in use. Notifying user.", port);
            return Err("PORT_IN_USE".into());
        }
    }

    let mut guard = state.child.lock().map_err(|_| "Failed to lock state")?;

    if guard.is_some() {
        return Err("Application is already running".into());
    }

    let project_path = state.current_path.lock().map_err(|_| "Failed to access path")?.clone();
    if project_path.to_string_lossy().is_empty() {
        return Err("Please select a project folder first".into());
    }
    println!("[KenBun] Starting Bun in: {:?}", project_path);

    // Jalankan perintah bun (menggunakan script path dari settings)
    let run_script = if script_path.is_empty() { "index.js".to_string() } else { script_path };
    
    // Preload Interceptor: buat file sementara yang memaksa port dari KenBun
    let preload_path = project_path.join(".kenbun-preload.js");
    if force_port {
        let preload_content = format!(
            "// KenBun Preload Interceptor - DO NOT EDIT\n\
             // This file is auto-generated and will be cleaned up automatically.\n\
             const _originalServe = Bun.serve;\n\
             Bun.serve = function(options) {{\n\
               const forcedPort = parseInt(process.env.PORT);\n\
               if (forcedPort && options.port !== forcedPort) {{\n\
                 console.log(`[KenBun Interceptor] Overriding port ${{options.port}} → ${{forcedPort}}`);\n\
                 options.port = forcedPort;\n\
               }}\n\
               return _originalServe.call(this, options);\n\
             }};\n"
        );
        std::fs::write(&preload_path, preload_content)
            .map_err(|e| format!("Failed to create preload script: {}", e))?;
        println!("[KenBun] Preload Interceptor created at: {:?}", preload_path);
    }

    let mode_label = if force_port { "FORCED" } else { "ADAPTIVE" };
    let log_msg = format!("Executing [{}]: PORT={} bun run {}", mode_label, port, run_script);
    println!("[KenBun] {}", log_msg);
    let _ = app.emit("log-event", log_msg);
    
    let mut cmd = std::process::Command::new("bun");
    
    if force_port && preload_path.exists() {
        let preload_str = preload_path.to_string_lossy().into_owned();
        cmd.args(["run", "--preload", &preload_str, &run_script]);
    } else {
        cmd.args(["run", &run_script]);
    }
    
    cmd.current_dir(&project_path)
        .env("PORT", port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            let stdout = child.stdout.take().ok_or("Gagal mengambil stdout")?;
            let stderr = child.stderr.take().ok_or("Gagal mengambil stderr")?;

            let app_clone = app.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                
                // Siapkan regex untuk deteksi port (dijalankan di luar loop agar efisien)
                let patterns = vec![
                    r"port\s*[:=]\s*(\d{4,5})",
                    r"listening on.*?(\d{4,5})",
                    r"started on.*?:\s*(\d{4,5})",
                    r"PORT\s*=\s*(\d{4,5})",
                    r"Server running at.*?(\d{4,5})",
                    r"Bun\.serve.*?port\s*:\s*(\d{4,5})",
                    r"localhost:(\d{4,5})"
                ];
                let regexes: Vec<regex::Regex> = patterns
                    .iter()
                    .filter_map(|p| regex::Regex::new(p).ok())
                    .collect();

                let mut port_detected = false;

                for line in reader.lines() {
                    if let Ok(l) = line {
                        if let Some(state) = app_clone.try_state::<AppState>() {
                            if let Ok(mut history) = state.log_history.lock() {
                                history.push(l.clone());
                                if history.len() > 1000 { history.remove(0); }
                            }
                        }
                        
                        // Emit log ke UI
                        let _ = app_clone.emit("log-event", l.clone());

                        // Cek port secara adaptif jika belum ketemu
                        if !port_detected {
                            for re in &regexes {
                                if let Some(caps) = re.captures(&l) {
                                    if let Ok(detected_port) = caps[1].parse::<u16>() {
                                        if detected_port >= 1024 {
                                            println!("[KenBun] Adaptive Resolver: Detected actual port {}", detected_port);
                                            let _ = app_clone.emit("port-detected", detected_port);
                                            port_detected = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });

            let app_clone_err = app.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(l) = line {
                        if let Some(state) = app_clone_err.try_state::<AppState>() {
                            if let Ok(mut history) = state.log_history.lock() {
                                history.push(l.clone());
                                if history.len() > 1000 { history.remove(0); }
                            }
                        }
                        let _ = app_clone_err.emit("log-event", l);
                    }
                }
            });

            // Simpan child agar bisa dimatikan nanti
            *guard = Some(child);
            
            // Monitor exit di thread terpisah tanpa memindahkan ownership
            let app_clone_wait = app.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    
                    let mut should_exit_loop = false;
                    
                    // Gunakan block terpisah agar lock segera dilepas
                    {
                        if let Some(state_handle) = app_clone_wait.try_state::<AppState>() {
                            if let Ok(mut guard) = state_handle.child.lock() {
                                if let Some(child) = guard.as_mut() {
                                    match child.try_wait() {
                                        Ok(Some(_status)) => {
                                            guard.take(); 
                                            should_exit_loop = true;
                                        }
                                        Ok(None) => {}
                                        Err(_) => {
                                            guard.take();
                                            should_exit_loop = true;
                                        }
                                    }
                                } else {
                                    should_exit_loop = true;
                                }
                            }
                        }
                    }

                    if should_exit_loop {
                        let _ = app_clone_wait.emit("process-exit", ());
                        break;
                    }
                }
            });

            Ok(pid)
        }
        Err(e) => Err(format!("Gagal menjalankan Bun: {}. Pastikan Bun terinstal.", e)),
    }
}

// ── Command: Matikan proses ──────────────────────────────────────────────────
#[tauri::command]
fn stop_bun(state: tauri::State<'_, AppState>, port: u16) -> Result<(), String> {
    let mut guard = state.child.lock().map_err(|_| "Gagal mengunci state")?;

    // Bersihkan file preload sementara jika ada
    if let Ok(project_path) = state.current_path.lock() {
        let preload_path = project_path.join(".kenbun-preload.js");
        if preload_path.exists() {
            let _ = std::fs::remove_file(&preload_path);
            println!("[KenBun] Cleaned up preload interceptor file.");
        }
    }

    if let Some(child) = guard.take() {
        kill_bun_process(child, port);
        Ok(())
    } else {
        cleanup_port(port);
        Ok(())
    }
}

#[tauri::command]
fn open_browser(app: tauri::AppHandle, url: String) {
    let _ = app.opener().open_url(url, None::<&str>);
}

// ── Command: Ambil riwayat log ───────────────────────────────────────────────
#[tauri::command]
fn get_log_history(state: tauri::State<'_, AppState>) -> Vec<String> {
    state.log_history.lock().map(|h| h.clone()).unwrap_or_default()
}

#[tauri::command]
fn get_project_path(state: tauri::State<'_, AppState>) -> String {
    let project_path = state.current_path.lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| std::path::PathBuf::from(""));
    
    let path_str = project_path.to_string_lossy().to_string();
    
    // Hilangkan prefix UNC Windows (\\?\) jika ada agar tampilan bersih
    path_str.replace("\\\\?\\", "")
}

#[tauri::command]
fn set_project_path(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let mut guard = state.current_path.lock().map_err(|_| "Gagal akses path")?;
    *guard = std::path::PathBuf::from(path);
    Ok(())
}

#[tauri::command]
fn get_bun_version() -> Result<String, String> {
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("cmd");
    #[cfg(windows)]
    cmd.args(["/C", "bun --version"]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    #[cfg(not(windows))]
    let mut cmd = std::process::Command::new("bun");
    #[cfg(not(windows))]
    cmd.arg("--version");

    match cmd.output() {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("[KenBun] Detected Bun Version: {}", version);
            if version.is_empty() {
                Ok("Not Found".to_string())
            } else {
                Ok(version)
            }
        },
        Err(e) => {
            println!("[KenBun] Error detecting Bun: {}", e);
            Ok("Not Found".to_string())
        },
    }
}

// ── Command: Buka Jendela Guide Framework ────────────────────────────────────
#[tauri::command]
async fn open_guide_window(handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = handle.get_webview_window("guide-window") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("guide-update", ()); // Sinyal agar jendela me-refresh data dari localStorage
        return Ok(());
    }

    let _guide_window = tauri::WebviewWindowBuilder::new(
        &handle,
        "guide-window",
        tauri::WebviewUrl::App("guide.html".into())
    )
    .title("KenBun — Port Configuration Guide")
    .inner_size(680.0, 640.0)
    .resizable(true)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn close_guide_window(handle: tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("guide-window") {
        let _ = window.close();
    }
}

#[tauri::command]
fn update_tray_menu(app: tauri::AppHandle, project_name: String, is_running: bool) {
    let toggle_text = if is_running { "Stop Server" } else { "Start Server" };
    let title_text = if project_name.is_empty() { "KenBun".to_string() } else { format!("KenBun: {}", project_name) };

    if let Ok(title_i) = tauri::menu::MenuItemBuilder::with_id("title", &title_text).enabled(false).build(&app) {
        if let Ok(sep) = tauri::menu::PredefinedMenuItem::separator(&app) {
            if let Ok(toggle_i) = tauri::menu::MenuItemBuilder::with_id("toggle", toggle_text).build(&app) {
                if let Ok(quit_i) = tauri::menu::MenuItemBuilder::with_id("quit", "Exit").build(&app) {
                    if let Ok(menu) = tauri::menu::MenuBuilder::new(&app)
                        .items(&[&title_i, &sep, &toggle_i, &quit_i])
                        .build() {
                            if let Some(tray) = app.tray_by_id("main") {
                                let _ = tray.set_menu(Some(menu));
                            }
                    }
                }
            }
        }
    }
}

// ── Command: Buka Jendela Log Terpisah ───────────────────────────────────────
#[tauri::command]
async fn open_log_window(handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = handle.get_webview_window("log-window") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let _log_window = tauri::WebviewWindowBuilder::new(
        &handle,
        "log-window",
        tauri::WebviewUrl::App("log.html".into())
    )
    .title("KenBun Terminal")
    .inner_size(800.0, 600.0)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn main() {
    let state = AppState {
        child: Mutex::new(None),
        log_history: Mutex::new(Vec::new()),
        current_path: Mutex::new(std::path::PathBuf::new()), // Kosongkan di awal
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            start_bun, 
            stop_bun, 
            open_browser,
            open_log_window,
            open_guide_window,
            close_guide_window,
            update_tray_menu,
            get_log_history,
            get_project_path,
            set_project_path,
            get_bun_version,
            verify_project
        ])
        .setup(|app| {
            let title_i = MenuItemBuilder::with_id("title", "KenBun")
                .enabled(false)
                .build(app)?;
            let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
            let toggle_i = MenuItemBuilder::with_id("toggle", "Start Server").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "Exit").build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&title_i, &sep, &toggle_i, &quit_i])
                .build()?;

            if let Some(icon) = app.default_window_icon() {
                let _tray = TrayIconBuilder::with_id("main")
                    .icon(icon.clone())
                    .tooltip("KenBun")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => {
                            if let Some(state) = app.try_state::<AppState>() {
                                if let Ok(mut guard) = state.child.lock() {
                                    if let Some(child) = guard.take() {
                                        kill_bun_process(child, 3000);
                                    } else {
                                        cleanup_port(3000);
                                    }
                                }
                            }
                            app.exit(0);
                        }
                        "toggle" => {
                            let _ = app.emit("tray-toggle", ());
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Destroyed => {
                    // Hanya matikan bun jika jendela utamanya yang hancur
                    if window.label() == "main" {
                        if let Ok(mut guard) = window.state::<AppState>().child.lock() {
                            if let Some(child) = guard.take() {
                                kill_bun_process(child, 3000);
                            }
                        }
                    }
                }
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Hanya sembunyikan jika jendela utama
                    if window.label() == "main" {
                        let _ = window.hide();
                        api.prevent_close();
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("Error saat menjalankan aplikasi Tauri");
}
