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

// ── State: Simpan Child process & Log History ───────────────────────────────
struct AppState {
    child: Mutex<Option<Child>>,
    log_history: Mutex<Vec<String>>,
}

// Fungsi untuk membersihkan port tertentu secara paksa di Windows
fn cleanup_port(port: u16) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", &format!("for /f \"tokens=5\" %a in ('netstat -aon ^| findstr :{}') do taskkill /F /PID %a", port)])
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

// ── Command: Jalankan bun index.js ──────────────────────────────────────────
#[tauri::command]
fn start_bun(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    port: u16,
) -> Result<u32, String> {
    cleanup_port(port);

    let mut guard = state.child.lock().map_err(|_| "Gagal mengunci state")?;

    if guard.is_some() {
        return Err("Aplikasi sudah berjalan".into());
    }

    let work_dir = if cfg!(debug_assertions) {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .parent()
            .unwrap_or(&std::path::PathBuf::from("."))
            .to_path_buf()
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    };

    let mut cmd = std::process::Command::new("bun");
    cmd.arg("index.js");
    cmd.current_dir(&work_dir);
    cmd.env("PORT", port.to_string());

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

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
                for line in reader.lines() {
                    if let Ok(l) = line {
                        if let Some(state) = app_clone.try_state::<AppState>() {
                            if let Ok(mut history) = state.log_history.lock() {
                                history.push(l.clone());
                                if history.len() > 1000 { history.remove(0); }
                            }
                        }
                        let _ = app_clone.emit("log-event", l);
                    }
                }
            });

            let app_clone_err = app.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(l) = line {
                        let msg = format!("ERROR: {}", l);
                        if let Some(state) = app_clone_err.try_state::<AppState>() {
                            if let Ok(mut history) = state.log_history.lock() {
                                history.push(msg.clone());
                                if history.len() > 1000 { history.remove(0); }
                            }
                        }
                        let _ = app_clone_err.emit("log-event", msg);
                    }
                }
            });

            *guard = Some(child);
            Ok(pid)
        }
        Err(e) => Err(format!("Gagal menjalankan Bun: {}. Pastikan Bun terinstal.", e)),
    }
}

// ── Command: Matikan proses ──────────────────────────────────────────────────
#[tauri::command]
fn stop_bun(state: tauri::State<'_, AppState>, port: u16) -> Result<(), String> {
    let mut guard = state.child.lock().map_err(|_| "Gagal mengunci state")?;

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
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            get_log_history
        ])
        .setup(|app| {
            let quit_i = MenuItemBuilder::with_id("quit", "Keluar").build(app)?;
            let show_i = MenuItemBuilder::with_id("show", "Tampilkan").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show_i, &quit_i]).build()?;

            if let Some(icon) = app.default_window_icon() {
                let _tray = TrayIconBuilder::new()
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
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
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
