// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod options;

use options::Options;

use std::{path::PathBuf, process::Stdio};

use tauri::{Emitter, Manager};

use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc::{Sender, channel},
};

const SETTINGS_PATH: &str = "settings.json";

fn get_settings_file_path(handle: &tauri::AppHandle) -> std::io::Result<PathBuf> {
    let mut path = handle
        .app_handle()
        .path()
        .app_config_dir()
        .unwrap_or_default();
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    path.push(SETTINGS_PATH);
    Ok(path)
}

#[tauri::command]
fn set_libpath(_: tauri::AppHandle) {
    if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_default();
        let libpath = format!("{home}/lib:/usr/local/lib:/usr/lib");
        let fallback_path = if let Ok(path) = std::env::var("DYLD_FALLBACK_LIBRARY_PATH") {
            if path.contains(&libpath) {
                path
            } else {
                format!("{path}:{libpath}")
            }
        } else {
            libpath
        };
        unsafe {
            std::env::set_var("DYLD_FALLBACK_LIBRARY_PATH", fallback_path);
        }
    }
}

#[tauri::command]
fn showfile(_: tauri::AppHandle, path: &str) {
    showfile::show_path_in_file_manager(path);
}

#[tauri::command]
async fn load_settings(handle: tauri::AppHandle) -> Result<Options, String> {
    let options: Options = if let Ok(mut file) =
        File::open(get_settings_file_path(&handle).map_err(|e| e.to_string())?).await
    {
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&contents).unwrap_or_default()
    } else {
        Default::default()
    };
    Ok(options)
}

#[tauri::command]
async fn save_settings(handle: tauri::AppHandle, options: &str) -> Result<(), String> {
    let options: Options = serde_json::from_str(options).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&options).map_err(|e| e.to_string())?;
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(get_settings_file_path(&handle).map_err(|e| e.to_string())?)
        .await
    {
        Ok(file) => file,
        Err(_) => File::create(get_settings_file_path(&handle).map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string())?,
    };
    file.write_all(json.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn wpcap_installed() -> bool {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            if libloading::Library::new("wpcap.dll").is_err() {
                return false;
            }
        }
    }
    true
}

#[tauri::command]
async fn twincat_installed() -> bool {
    std::path::Path::new("C:/Program Files (x86)/Beckhoff/TwinCAT/3.1/Config/Io/EtherCAT").exists()
}

#[tauri::command]
async fn copy_autd_xml(
    handle: tauri::AppHandle,
    console_emu_input_tx: tauri::State<'_, Sender<String>>,
) -> Result<(), String> {
    let dst = std::path::Path::new(
        "C:/Program Files (x86)/Beckhoff/TwinCAT/3.1/Config/Io/EtherCAT/AUTD.xml",
    );

    if dst.exists() {
        console_emu_input_tx
            .send("AUTD.xml is already exists".to_string())
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    if dst.parent().is_some_and(|p| !p.exists()) {
        return Err("TwinCAT is not installed".to_string());
    }

    let autd_xml_path = handle
        .path()
        .resource_dir()
        .map(|resource| resource.join("TwinCATAUTDServer/AUTD.xml"))
        .map_err(|_| "Can't find AUTD.xml")?;

    tokio::fs::copy(autd_xml_path, dst)
        .await
        .map_err(|e| format!("{e}: please run as administrator"))?;

    console_emu_input_tx
        .send("AUTD.xml is successfully copied".to_string())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn run_twincat_server(
    twincat_options: &str,
    handle: tauri::AppHandle,
    console_emu_input_tx: tauri::State<'_, Sender<String>>,
) -> Result<(), String> {
    let twincat_autd_server_path = handle
        .path()
        .resource_dir()
        .map(|resource| resource.join("TwinCATAUTDServer/TwinCATAUTDServer.exe"))
        .map_err(|_| "Can't find TwinCATAUTDServer.exe")?;

    let twincat_options: options::TwinCATOptions =
        serde_json::from_str(twincat_options).map_err(|e| e.to_string())?;

    let mut args = vec![
        "-c".to_string(),
        twincat_options.client,
        "--device_name".to_string(),
        twincat_options.device_name,
        "--twincat".to_string(),
        twincat_options.version.to_string(),
        "-s".to_string(),
        twincat_options.sync0.to_string(),
        "-t".to_string(),
        twincat_options.task.to_string(),
        "-b".to_string(),
        twincat_options.base.to_string(),
    ];
    if twincat_options.keep {
        args.push("-k".to_string());
    }
    if twincat_options.debug {
        args.push("--debug".to_string());
    }

    #[cfg(target_os = "windows")]
    let mut child = Command::new(&twincat_autd_server_path)
        .args(args)
        .stdout(Stdio::piped())
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "windows"))]
    let mut child = Command::new(&twincat_autd_server_path)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let mut reader = BufReader::new(stdout);

    loop {
        let mut buf = String::new();
        if reader.read_line(&mut buf).await.unwrap() == 0 {
            break;
        }
        console_emu_input_tx
            .send(buf.trim().to_string())
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn open_xae_shell() -> Result<(), String> {
    let path = std::env::var("TEMP").unwrap_or_default();
    let path = std::path::Path::new(&path)
        .join("TwinCATAUTDServer")
        .join("TwinCATAUTDServer.sln");

    let xae_shell = std::path::Path::new("C:\\")
        .join("Program Files")
        .join("Beckhoff")
        .join("TcXaeShell")
        .join("Common7")
        .join("IDE")
        .join("TcXaeShell.exe");

    if path.exists() {
        Command::new(&xae_shell).arg(&path).spawn()
    } else {
        Command::new(&xae_shell).spawn()
    }
    .map_err(|e| e.to_string())?
    .wait()
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tauri::async_runtime::set(tokio::runtime::Handle::current());

    let (console_emu_input_tx, mut console_emu_input_rx) = channel::<String>(32);

    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .manage(console_emu_input_tx)
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
                window.close_devtools();
            }

            let app_handle = app.handle().clone();
            tokio::spawn(async move {
                while let Some(s) = console_emu_input_rx.recv().await {
                    app_handle.emit("console-emu", s).unwrap();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_libpath,
            showfile,
            load_settings,
            save_settings,
            copy_autd_xml,
            run_twincat_server,
            open_xae_shell,
            twincat_installed,
            wpcap_installed
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
