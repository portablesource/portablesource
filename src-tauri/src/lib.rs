use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::fs;
use winreg::enums::*;
use winreg::RegKey;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use tauri::Emitter;
use std::sync::Mutex;

// Integrate Rust CLI library directly
use portablesource_rs::config::ConfigManager as PsConfigManager;
use portablesource_rs::envs_manager::PortableEnvironmentManager as PsEnvManager;
use portablesource_rs::repository_installer::RepositoryInstaller as PsRepoInstaller;
use portablesource_rs::utils as ps_utils;

// Keep shared config to reduce redundant disk I/O
struct AppState { config: Mutex<PsConfigManager> }

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[tauri::command]
async fn proxy_request(url: String) -> Result<String, String> {
    let response = reqwest::get(&url)
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Request failed with status: {}", response.status()));
    }

    let body = response.text().await.map_err(|e| e.to_string())?;
    Ok(body)
}


#[derive(Debug, Serialize, Deserialize)]
struct InstallResult {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandResult {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

#[derive(Clone, serde::Serialize)]
struct StreamOutput {
    stream: String, // "stdout" or "stderr"
    data: String,
}

#[derive(Clone, serde::Serialize)]
struct StreamFinished {
    success: bool,
    exit_code: Option<i32>,
}

#[derive(Clone, serde::Serialize)]
struct UiProgressEvent {
    phase: String,      // tool key: python/git/ffmpeg/cuda or "init"/"done"
    done: usize,
    total: usize,
}

#[tauri::command]
async fn set_install_path(path: String) -> Result<InstallResult, String> {
    // Normalize to include leaf 'portablesource' folder to ensure stable structure
    let mut target = PathBuf::from(&path);
    if !target
        .file_name()
        .and_then(|s| s.to_str())
        .map(|n| n.eq_ignore_ascii_case("portablesource"))
        .unwrap_or(false)
    {
        target = target.join("portablesource");
    }

    fs::create_dir_all(&target).map_err(|e| format!("Failed to create directory: {}", e))?;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .create_subkey("Software\\PortableSource")
        .map_err(|e| format!("Failed to create registry key: {}", e))?;

    key.0
        .set_value("InstallPath", &target.to_string_lossy().to_string())
        .map_err(|e| format!("Failed to set registry value: {}", e))?;

    Ok(InstallResult {
        success: true,
        message: "Installation path saved successfully".to_string(),
        normalized_path: Some(target.to_string_lossy().to_string()),
    })
}

#[tauri::command]
async fn get_install_path() -> Result<String, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey("Software\\PortableSource")
        .map_err(|_| "Registry key not found".to_string())?;
    
    let path: String = key.get_value("InstallPath")
        .map_err(|_| "Install path not found in registry".to_string())?;
    
    Ok(path)
}

#[tauri::command]
async fn find_cli_installation() -> Result<String, String> {
    // 1) Попытка через реестр
    if let Ok(path) = get_install_path().await {
        let root = Path::new(&path);
        if root.exists() { return Ok(path); }
    }

    // 2) Эвристика по типичным путям нашей программы
    let candidates = [
        PathBuf::from(r"C:\PS"),
        PathBuf::from(r"C:\PortableSource"),
    ];
    for c in candidates.iter() {
        if c.exists() {
            // признак установки: есть ps_env или конфиг
            let ps_env = c.join("ps_env");
            let conf_in_root = c.join("portablesource_config.json");
            if ps_env.exists() || conf_in_root.exists() {
                return Ok(c.to_string_lossy().to_string());
            }
        }
    }

    // 3) Попытка прочитать конфиг из корня C:\PS\portablesource_config.json, если есть
    let c_ps_cfg = Path::new(r"C:\PS\portablesource_config.json");
    if c_ps_cfg.exists() {
        if let Ok(text) = std::fs::read_to_string(c_ps_cfg) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(install) = json.get("install_path").and_then(|v| v.as_str()) {
                    let p = PathBuf::from(install);
                    if p.exists() { return Ok(p.to_string_lossy().to_string()); }
                }
            }
        }
    }

    Err("Installation path not found".to_string())
}

#[tauri::command]
async fn download_and_install_cli(state: tauri::State<'_, AppState>, install_path: String) -> Result<InstallResult, String> {
    let mut install_dir = PathBuf::from(&install_path);
    if !install_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|n| n.eq_ignore_ascii_case("portablesource"))
        .unwrap_or(false)
    {
        install_dir = install_dir.join("portablesource");
    }
    fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create install directory: {}", e))?;
    
    // Сохраняем путь в реестр, создаём структуру каталогов и настраиваем окружение через библиотеку
    ps_utils::save_install_path_to_registry(&install_dir)
        .map_err(|e| e.to_string())?;
    ps_utils::create_directory_structure(&install_dir)
        .map_err(|e| e.to_string())?;

    let mut cfg = state.config.lock().map_err(|_| "State poisoned")?.clone();
    if cfg.get_config().install_path.as_os_str().is_empty() {
        // Устанавливаем один раз без повторных сохранений в циклах
        cfg.set_install_path(install_dir.clone()).map_err(|e| e.to_string())?;
    }

    let env_mgr = PsEnvManager::with_config(install_dir.clone(), cfg.clone());
    env_mgr.setup_environment().await.map_err(|e| e.to_string())?;

    // Reload config from disk to reflect changes performed by environment setup
    if let Ok(updated) = PsConfigManager::new(None) {
        *state.config.lock().map_err(|_| "State poisoned")? = updated;
    } else {
        *state.config.lock().map_err(|_| "State poisoned")? = cfg;
    }
    Ok(InstallResult { success: true, message: "Environment installed successfully".to_string(), normalized_path: Some(install_dir.to_string_lossy().to_string()) })
}

#[tauri::command]
async fn test_cli_installation(state: tauri::State<'_, AppState>, install_path: String) -> Result<InstallResult, String> {
    let install_dir = PathBuf::from(install_path);
    if !install_dir.exists() {
        return Ok(InstallResult { success: false, message: "Install path not found".into(), normalized_path: None });
    }
    // Проверим доступность базовых каталогов/конфига через библиотеку
    let _guard = state.config.lock().map_err(|_| "State poisoned")?;
    Ok(InstallResult { success: true, message: "Library available".into(), normalized_path: Some(install_dir.to_string_lossy().to_string()) })
}

#[tauri::command]
async fn run_cli_command(state: tauri::State<'_, AppState>, install_path: String, args: Vec<String>) -> Result<CommandResult, String> {
    let install_dir = PathBuf::from(&install_path);
    let mut cfg = state.config.lock().map_err(|_| "State poisoned")?.clone();
    if cfg.get_config().install_path.as_os_str().is_empty() {
        cfg.set_install_path(install_dir.clone()).map_err(|e| e.to_string())?;
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut success = true;

    if args.contains(&"--setup-env".to_string()) {
        let env_mgr = PsEnvManager::with_config(install_dir.clone(), cfg.clone());
        match env_mgr.setup_environment().await {
            Ok(_) => { stdout.push_str("Environment setup completed successfully\n"); }
            Err(e) => { success = false; stderr.push_str(&e.to_string()); }
        }
    } else if let Some(pos) = args.iter().position(|a| a == "--install-repo") {
        if let Some(repo) = args.get(pos + 1) {
            let mut installer = PsRepoInstaller::new(install_dir.clone(), cfg.clone());
            match installer.install_repository(repo).await {
                Ok(_) => stdout.push_str("Repository installed successfully\n"),
                Err(e) => { success = false; stderr.push_str(&e.to_string()); }
            }
        } else { success = false; stderr.push_str("Missing repository name"); }
    } else if let Some(pos) = args.iter().position(|a| a == "--update-repo") {
        if let Some(repo) = args.get(pos + 1) {
            let mut installer = PsRepoInstaller::new(install_dir.clone(), cfg.clone());
            match installer.update_repository(repo).await {
                Ok(_) => stdout.push_str("Repository updated successfully\n"),
                Err(e) => { success = false; stderr.push_str(&e.to_string()); }
            }
        } else { success = false; stderr.push_str("Missing repository name"); }
    } else if let Some(pos) = args.iter().position(|a| a == "--delete-repo") {
        if let Some(repo) = args.get(pos + 1) {
            let installer = PsRepoInstaller::new(install_dir.clone(), cfg.clone());
            match installer.delete_repository(repo) {
                Ok(_) => stdout.push_str("Repository deleted successfully\n"),
                Err(e) => { success = false; stderr.push_str(&e.to_string()); }
            }
        } else { success = false; stderr.push_str("Missing repository name"); }
    } else if args.contains(&"--version".to_string()) {
        stdout.push_str(&format!("PortableSource version: {}\n", portablesource_rs::config::VERSION));
    } else if args.contains(&"--check-env".to_string()) {
        let env_mgr = PsEnvManager::with_config(install_dir.clone(), cfg.clone());
        match env_mgr.check_environment_status() {
            Ok(status) => {
                stdout.push_str(&format!("Environment exists: {}\n", if status { "YES" } else { "NO" }));
                stdout.push_str(&format!("Setup completed: {}\n", if cfg.is_environment_setup_completed() { "YES" } else { "NO" }));
            }
            Err(e) => { success = false; stderr.push_str(&e.to_string()); }
        }
    } else {
        success = false;
        stderr.push_str("Unsupported command arguments");
    }

    // Refresh config from disk to pick persisted changes if any
    if let Ok(updated) = PsConfigManager::new(None) {
        *state.config.lock().map_err(|_| "State poisoned")? = updated;
    } else {
        *state.config.lock().map_err(|_| "State poisoned")? = cfg;
    }
    Ok(CommandResult { success, stdout, stderr, exit_code: Some(if success { 0 } else { 1 }) })
}

#[tauri::command]
async fn run_command_stream(
    app_handle: tauri::AppHandle,
    command: String,
    working_dir: Option<String>,
    event_id: String,
) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("powershell");
        cmd.args(["-Command", &command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &command]);
        cmd
    };
    
    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }
    
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // Hide console window on Windows
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    
    let mut child = cmd.spawn()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;
    
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    
    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);
    
    let app_handle_stdout = app_handle.clone();
    let event_id_stdout = event_id.clone();
    
    // Handle stdout in a separate task
    let stdout_task = tokio::spawn(async move {
        for line in stdout_reader.lines() {
            if let Ok(line) = line {
                let _ = app_handle_stdout.emit(
                    &format!("command-output-{}", event_id_stdout),
                    StreamOutput {
                        stream: "stdout".to_string(),
                        data: line,
                    },
                );
            }
        }
    });
    
    let app_handle_stderr = app_handle.clone();
    let event_id_stderr = event_id.clone();
    
    // Handle stderr in a separate task
    let stderr_task = tokio::spawn(async move {
        for line in stderr_reader.lines() {
            if let Ok(line) = line {
                let _ = app_handle_stderr.emit(
                    &format!("command-output-{}", event_id_stderr),
                    StreamOutput {
                        stream: "stderr".to_string(),
                        data: line,
                    },
                );
            }
        }
    });
    
    // Wait for the process to finish
    let status = child.wait().map_err(|e| format!("Failed to wait for command: {}", e))?;
    
    // Wait for both tasks to complete
    let _ = tokio::join!(stdout_task, stderr_task);
    
    // Emit completion event
    let _ = app_handle.emit(
        &format!("command-finished-{}", event_id),
        StreamFinished {
            success: status.success(),
            exit_code: status.code(),
        },
    );
    
    Ok(())
}

#[tauri::command]
async fn run_cli_command_stream(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    install_path: String,
    args: Vec<String>,
    event_id: String,
) -> Result<(), String> {
    // Эмуляция потокового вывода поверх библиотечных вызовов
    let install_dir = PathBuf::from(&install_path);
    let mut cfg = state.config.lock().map_err(|_| "State poisoned")?.clone();
    if cfg.get_config().install_path.as_os_str().is_empty() {
        let _ = cfg.set_install_path(install_dir.clone());
    }

    let app = app_handle.clone();
    let ev_id = event_id.clone();

    let emit_line = move |stream: &str, data: String| {
        let _ = app.emit(&format!("cli-output-{}", ev_id), StreamOutput { stream: stream.to_string(), data });
    };

    emit_line("stdout", format!("Starting: {:?}", args));

    let mut success = true;
    let mut exit_code: Option<i32> = Some(0);

    if args.contains(&"--setup-env".to_string()) {
        emit_line("stdout", "Setting up environment...".into());
        let env_mgr = PsEnvManager::new(install_dir.clone());
        if let Err(e) = env_mgr.setup_environment().await { success = false; exit_code = Some(1); emit_line("stderr", e.to_string()); }
        else { emit_line("stdout", "Environment setup completed".into()); }
    } else if let Some(pos) = args.iter().position(|a| a == "--install-repo") {
        if let Some(repo) = args.get(pos + 1) {
            emit_line("stdout", format!("Installing repo '{}'...", repo));
            let mut installer = PsRepoInstaller::new(install_dir.clone(), cfg.clone());
            if let Err(e) = installer.install_repository(repo).await { success = false; exit_code = Some(1); emit_line("stderr", e.to_string()); }
            else { emit_line("stdout", "Repository installed".into()); }
        } else { success = false; exit_code = Some(1); emit_line("stderr", "Missing repository name".into()); }
    } else if let Some(pos) = args.iter().position(|a| a == "--update-repo") {
        if let Some(repo) = args.get(pos + 1) {
            emit_line("stdout", format!("Updating repo '{}'...", repo));
            let mut installer = PsRepoInstaller::new(install_dir.clone(), cfg.clone());
            if let Err(e) = installer.update_repository(repo).await { success = false; exit_code = Some(1); emit_line("stderr", e.to_string()); }
            else { emit_line("stdout", "Repository updated".into()); }
        } else { success = false; exit_code = Some(1); emit_line("stderr", "Missing repository name".into()); }
    } else if let Some(pos) = args.iter().position(|a| a == "--delete-repo") {
        if let Some(repo) = args.get(pos + 1) {
            emit_line("stdout", format!("Deleting repo '{}'...", repo));
            let installer = PsRepoInstaller::new(install_dir.clone(), cfg.clone());
            if let Err(e) = installer.delete_repository(repo) { success = false; exit_code = Some(1); emit_line("stderr", e.to_string()); }
            else { emit_line("stdout", "Repository deleted".into()); }
        } else { success = false; exit_code = Some(1); emit_line("stderr", "Missing repository name".into()); }
    } else if args.contains(&"--version".to_string()) {
        emit_line("stdout", format!("PortableSource version: {}", portablesource_rs::config::VERSION));
    } else if args.contains(&"--check-env".to_string()) {
        let env_mgr = PsEnvManager::new(install_dir.clone());
        match env_mgr.check_environment_status() {
            Ok(status) => {
                emit_line("stdout", format!("Environment exists: {}", if status { "YES" } else { "NO" }));
                emit_line("stdout", format!("Setup completed: {}", if cfg.is_environment_setup_completed() { "YES" } else { "NO" }));
            }
            Err(e) => { success = false; exit_code = Some(1); emit_line("stderr", e.to_string()); }
        }
    } else {
        success = false;
        exit_code = Some(1);
        emit_line("stderr", "Unsupported command arguments".into());
    }

    *state.config.lock().map_err(|_| "State poisoned")? = cfg;
    let _ = app_handle.emit(
        &format!("cli-finished-{}", event_id),
        StreamFinished { success, exit_code },
    );
    Ok(())
}

#[tauri::command]
async fn setup_environment_stream(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    install_path: String,
    event_id: String,
) -> Result<(), String> {
    log::info!("setup_environment_stream(install_path={}, event_id={})", install_path, event_id);
    let install_dir = std::path::PathBuf::from(&install_path);
    let mut cfg = state.config.lock().map_err(|_| "State poisoned")?.clone();
    if cfg.get_config().install_path.as_os_str().is_empty() {
        let _ = cfg.set_install_path(install_dir.clone());
    }
    let env_mgr = PsEnvManager::with_config(install_dir.clone(), cfg.clone());

    let app = app_handle.clone();
    let ev_id = event_id.clone();
    let app_progress = app.clone();
    let ev_progress = ev_id.clone();
    let emit = move |phase: String, done: usize, total: usize| {
        let _ = app_progress.emit(
            &format!("env-setup-progress-{}", ev_progress),
            UiProgressEvent { phase, done, total },
        );
    };

    let mut success = true;
    if let Err(e) = env_mgr
        .setup_environment_with_progress(move |phase, done, total| emit(phase, done, total))
        .await
    {
        success = false;
        let _ = app_handle.emit(
            &format!("env-setup-error-{}", event_id),
            e.to_string(),
        );
    }
    let _ = app_handle.emit(
        &format!("env-setup-finished-{}", event_id),
        StreamFinished { success, exit_code: Some(if success { 0 } else { 1 }) },
    );
    // Reload config so in-memory state matches file after setup
    if let Ok(updated) = PsConfigManager::new(None) {
        *state.config.lock().map_err(|_| "State poisoned")? = updated;
    } else {
        *state.config.lock().map_err(|_| "State poisoned")? = cfg;
    }
    Ok(())
}

#[tauri::command]
async fn run_batch_in_new_window(batch_file: String, working_dir: String) -> Result<CommandResult, String> {
    if cfg!(target_os = "windows") {
        // Use start command to open batch file in new console window
        let output = Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", &batch_file])
            .current_dir(&working_dir)
            .output()
            .map_err(|e| format!("Failed to run batch file in new window: {}", e))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();
        
        Ok(CommandResult {
            success: output.status.success(),
            stdout,
            stderr,
            exit_code,
        })
    } else {
        Err("This function is only supported on Windows".to_string())
    }
}

#[tauri::command]
async fn check_environment_installed(install_path: String) -> Result<bool, String> {
    let conda_path = Path::new(&install_path).join("miniconda").join("_conda.exe");
    Ok(conda_path.exists())
}

#[tauri::command]
async fn check_repository_installed(install_path: String, repo_name: String) -> Result<bool, String> {
    let repo_path = Path::new(&install_path).join("repos").join(&repo_name);
    Ok(repo_path.exists() && repo_path.is_dir())
}

#[tauri::command]
async fn list_directory_folders(install_path: String, directory_name: String) -> Result<Vec<String>, String> {
    let dir_path = Path::new(&install_path).join(&directory_name);
    
    if !dir_path.exists() || !dir_path.is_dir() {
        return Ok(vec![]);
    }
    
    let mut folders = Vec::new();
    
    match fs::read_dir(&dir_path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(folder_name) = path.file_name() {
                            if let Some(name_str) = folder_name.to_str() {
                                // Skip hidden folders (starting with .)
                                if !name_str.starts_with('.') {
                                    folders.push(name_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            return Err(format!("Failed to read directory {}: {}", dir_path.display(), e));
        }
    }
    
    folders.sort();
    Ok(folders)
}

#[tauri::command]
async fn run_command(command: String, working_dir: Option<String>) -> Result<CommandResult, String> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("powershell");
        cmd.args(["-Command", &command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &command]);
        cmd
    };
    
    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }
    
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // Hide console window on Windows
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to execute command: {}", e))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code();
    
    Ok(CommandResult {
        success: output.status.success(),
        stdout,
        stderr,
        exit_code,
    })
}

#[derive(Clone, serde::Serialize)]
struct EnvironmentStatus {
    environment_exists: bool,
    setup_completed: bool,
    overall_status: String,
}

#[tauri::command]
async fn check_environment_status(state: tauri::State<'_, AppState>, install_path: String) -> Result<EnvironmentStatus, String> {
    //log::info!("check_environment_status(install_path={})", install_path);
    let install_dir = PathBuf::from(&install_path);
    let mut cfg = state.config.lock().map_err(|_| "State poisoned")?.clone();
    let env_mgr = PsEnvManager::with_config(install_dir.clone(), cfg.clone());
    let environment_exists = env_mgr.check_environment_status().map_err(|e| e.to_string())?;
    if cfg.get_config().install_path.as_os_str().is_empty() {
        let _ = cfg.set_install_path(install_dir.clone());
    }
    let setup_completed = cfg.is_environment_setup_completed();
    let overall_status = if setup_completed {
        "Ready".to_string()
    } else if environment_exists {
        "Environment exists but setup incomplete".to_string()
    } else {
        "Environment not found".to_string()
    };
    *state.config.lock().map_err(|_| "State poisoned")? = cfg;
    Ok(EnvironmentStatus { environment_exists, setup_completed, overall_status })
}

#[tauri::command]
async fn clear_install_path() -> Result<InstallResult, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    
    match hkcu.open_subkey("Software\\PortableSource") {
        Ok(key) => {
            key.delete_value("InstallPath")
                .map_err(|e| format!("Failed to delete registry value: {}", e))?;
            
            Ok(InstallResult {
                success: true,
                message: "Installation path cleared successfully".to_string(),
                normalized_path: None,
            })
        }
        Err(_) => {
            Ok(InstallResult {
                success: true,
                message: "Registry key not found, nothing to clear".to_string(),
                normalized_path: None,
            })
        }
    }
}

#[tauri::command]
async fn delete_repository(state: tauri::State<'_, AppState>, install_path: String, repo_name: Option<String>) -> Result<InstallResult, String> {
    log::info!("[tauri] delete_repository called: install_path={:?}, repo_name={:?}", install_path, repo_name);
    let repo_name_for_message = repo_name.clone().unwrap_or_default();
    if let Some(repo) = repo_name {
        let mut cfg = state.config.lock().map_err(|_| "State poisoned")?.clone();
        let install_dir = PathBuf::from(&install_path);
        if cfg.get_config().install_path.as_os_str().is_empty() {
            let _ = cfg.set_install_path(install_dir.clone());
        }
        let installer = PsRepoInstaller::new(install_dir.clone(), cfg.clone());
        let result = installer.delete_repository(&repo);
        if result.is_ok() {
            // Try to remove folders from envs/ and repos/
            let envs = install_dir.join("envs").join(&repo);
            let repos = install_dir.join("repos").join(&repo);
            let _ = std::fs::remove_dir_all(&envs);
            let _ = std::fs::remove_dir_all(&repos);
        }
        *state.config.lock().map_err(|_| "State poisoned")? = cfg;
        match result {
            Ok(_) => Ok(InstallResult { success: true, message: format!("Repository '{}' deleted successfully", repo_name_for_message), normalized_path: None }),
            Err(e) => Ok(InstallResult { success: false, message: format!("Failed to delete repository: {}", e), normalized_path: None }),
        }
    } else {
        Ok(InstallResult { success: false, message: "Use removeAllRepos function for deleting all repositories".into(), normalized_path: None })
    }
}

#[tauri::command]
async fn get_cli_version(_state: tauri::State<'_, AppState>, install_path: String) -> Result<String, String> {
    // Log to ensure dev build picks new signature
    log::info!("get_cli_version called with install_path={}", install_path);
    Ok(portablesource_rs::config::VERSION.to_string())
}

#[tauri::command]
async fn get_latest_version_from_github() -> Result<String, String> {
    let url = "https://api.github.com/repos/portablesource/portablesource-cli/releases/latest";
    
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "PortableSource-App/1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch latest version: {}", e))?;
    
    if response.status() == 403 {
        // GitHub API rate limit exceeded, return error instead of fallback version
        return Err("GitHub API rate limit exceeded. Cannot check for updates.".to_string());
    }
    
    if !response.status().is_success() {
        return Err(format!("GitHub API request failed with status: {}", response.status()));
    }
    
    let json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("Failed to parse GitHub API response: {}", e))?;
    
    if let Some(tag_name) = json["tag_name"].as_str() {
        // Remove 'v' prefix if present
        let version = tag_name.strip_prefix('v').unwrap_or(tag_name);
        Ok(version.to_string())
    } else {
        Err("Could not find tag_name in GitHub API response".to_string())
    }
}

#[tauri::command]
async fn check_for_updates(app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    use tauri_plugin_updater::UpdaterExt;
    
    match app_handle.updater() {
        Ok(updater) => {
            match updater.check().await {
                Ok(update) => {
                    if let Some(update) = update {
                        Ok(serde_json::json!({
                            "available": true,
                            "version": update.version,
                            "date": update.date.map(|d| d.to_string()),
                            "body": update.body
                        }))
                    } else {
                        Ok(serde_json::json!({
                            "available": false
                        }))
                    }
                },
                Err(e) => Err(format!("Failed to check for updates: {}", e))
            }
        },
        Err(e) => Err(format!("Updater not available: {}", e))
    }
}

#[tauri::command]
async fn install_update(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    
    match app_handle.updater() {
        Ok(updater) => {
            match updater.check().await {
                Ok(update) => {
                    if let Some(update) = update {
                        match update.download_and_install(|_chunk_length, _content_length| {}, || {}).await {
                            Ok(_) => Ok(()),
                            Err(e) => Err(format!("Failed to install update: {}", e))
                        }
                    } else {
                        Err("No update available".to_string())
                    }
                },
                Err(e) => Err(format!("Failed to check for updates: {}", e))
            }
        },
        Err(e) => Err(format!("Updater not available: {}", e))
    }
}

// --- MSVC Build Tools support & admin check ---
#[tauri::command]
async fn check_msvc_bt_installed() -> Result<bool, String> {
    Ok(ps_utils::check_msvc_build_tools_installed())
}

#[tauri::command]
async fn install_msvc_bt() -> Result<(), String> {
    ps_utils::install_msvc_build_tools().map_err(|e| e.to_string())
}

#[tauri::command]
async fn is_admin() -> Result<bool, String> {
    // Windows PowerShell check; on other OS always false
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let output = Command::new("powershell")
            .args([
                "-Command",
                "[bool]([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)"
            ])
            .output()
            .map_err(|e| format!("Failed to run PowerShell: {}", e))?;
        let out = String::from_utf8_lossy(&output.stdout).to_string();
        let is_admin = out.to_lowercase().contains("true");
        return Ok(is_admin);
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

#[tauri::command]
async fn get_system_locale() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        
        // Try to get locale from Control Panel\International
        if let Ok(intl_key) = hkcu.open_subkey("Control Panel\\International") {
            if let Ok(locale_name) = intl_key.get_value::<String, _>("LocaleName") {
                // Convert Windows locale format to standard format
                let locale = locale_name.replace("-", "_").to_lowercase();
                
                // Map common locales to our supported ones
                if locale.starts_with("ru") {
                    return Ok("ru".to_string());
                } else {
                    return Ok("en".to_string());
                }
            }
        }
        
        // Fallback: try to get from user default locale
        if let Ok(intl_key) = hkcu.open_subkey("Control Panel\\International") {
            if let Ok(locale) = intl_key.get_value::<String, _>("Locale") {
                // Russian locale codes
                if locale == "00000419" || locale == "0419" {
                    return Ok("ru".to_string());
                }
            }
        }
        
        // Default to English
        Ok("en".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // For non-Windows systems, default to English
        Ok("en".to_string())
    }
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
async fn complete_uninstall(state: tauri::State<'_, AppState>) -> Result<InstallResult, String> {
    // First, get the install path
    let install_path = match get_install_path().await {
        Ok(path) => path,
        Err(_) => {
            return Ok(InstallResult {
                success: false,
                message: "Installation path not found in registry".to_string(),
                normalized_path: None,
            });
        }
    };
    
    let install_dir = Path::new(&install_path);
    // Шаг 1: Очистить ключи реестра (без вызова внешнего EXE)
    let _ = clear_install_path().await;
    
    // Step 2: Remove the entire installation directory
    if install_dir.exists() {
        match fs::remove_dir_all(&install_dir) {
            Ok(_) => {
                // Step 3: Clear registry entry (повторно на всякий случай)
                let _ = clear_install_path().await;
                // Reset in-memory config
                *state.config.lock().map_err(|_| "State poisoned")? = PsConfigManager::new(None).map_err(|e| e.to_string())?;
                Ok(InstallResult {
                    success: true,
                    message: "Thank you for using this software! =}".to_string(),
                    normalized_path: None,
                })
            }
            Err(e) => {
                Ok(InstallResult {
                    success: false,
                    message: format!("Failed to remove installation directory: {}", e),
                    normalized_path: None,
                })
            }
        }
    } else {
        // Directory doesn't exist, just clear registry
        let _ = clear_install_path().await;
        
        Ok(InstallResult {
            success: true,
            message: "Thank you for using this software! =}".to_string(),
            normalized_path: None,
        })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState { config: Mutex::new(PsConfigManager::new(None).unwrap_or_else(|_| PsConfigManager::new(Some(PathBuf::from("."))).expect("config init"))) })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            app.handle().plugin(tauri_plugin_dialog::init())?;
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_install_path,
            get_install_path,
            find_cli_installation,
            download_and_install_cli,
            test_cli_installation,
            run_cli_command,
            proxy_request,
            clear_install_path,
            delete_repository,
            complete_uninstall,
            check_environment_installed,
            check_environment_status,
            check_repository_installed,
            list_directory_folders,
            run_command,
            run_command_stream,
            run_cli_command_stream,
            setup_environment_stream,
            run_batch_in_new_window,
            get_cli_version,
            get_latest_version_from_github,
            get_system_locale,
            get_app_version,
            check_for_updates,
            install_update,
            check_msvc_bt_installed,
            install_msvc_bt,
            is_admin
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
