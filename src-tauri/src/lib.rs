use std::path::Path;
use std::process::{Command, Stdio};
use std::fs;
use winreg::enums::*;
use winreg::RegKey;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use tauri::Emitter;

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

#[tauri::command]
async fn set_install_path(path: String) -> Result<InstallResult, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.create_subkey("Software\\PortableSource")
        .map_err(|e| format!("Failed to create registry key: {}", e))?;
    
    key.0.set_value("InstallPath", &path)
        .map_err(|e| format!("Failed to set registry value: {}", e))?;
    
    Ok(InstallResult {
        success: true,
        message: "Installation path saved successfully".to_string(),
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
    // Get path from registry (CLI saves it there)
    let path = get_install_path().await?;
    let exe_path = Path::new(&path).join("portablesource_main.exe");
    
    if exe_path.exists() {
        Ok(path)
    } else {
        Err("CLI executable not found at registered path".to_string())
    }
}

#[tauri::command]
async fn download_and_install_cli(install_path: String) -> Result<InstallResult, String> {
    let install_dir = Path::new(&install_path);
    
    // Create install directory if it doesn't exist
    fs::create_dir_all(install_dir)
        .map_err(|e| format!("Failed to create install directory: {}", e))?;
    
    let zip_path = install_dir.join("portablesource-Windows.zip");
    let url = "https://github.com/portablesource/portablesource-cli/releases/latest/download/portablesource-Windows.zip";
    
    // Download the CLI zip file
    let response = reqwest::get(url).await
        .map_err(|e| format!("Failed to download CLI: {}", e))?;
    
    let bytes = response.bytes().await
        .map_err(|e| format!("Failed to read download data: {}", e))?;
    
    fs::write(&zip_path, bytes)
        .map_err(|e| format!("Failed to save zip file: {}", e))?;
    
    // Extract the zip file
    let file = fs::File::open(&zip_path)
        .map_err(|e| format!("Failed to open zip file: {}", e))?;
    
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip archive: {}", e))?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read file from archive: {}", e))?;
        
        let outpath = match file.enclosed_name() {
            Some(path) => install_dir.join(path),
            None => continue,
        };
        
        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)
                        .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                }
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create output file: {}", e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }
    }
    
    // Remove the zip file
    fs::remove_file(&zip_path).ok();
    
    Ok(InstallResult {
        success: true,
        message: "CLI downloaded and extracted successfully".to_string(),
    })
}

#[tauri::command]
async fn test_cli_installation(install_path: String) -> Result<InstallResult, String> {
    let exe_path = Path::new(&install_path).join("portablesource_main.exe");
    
    if !exe_path.exists() {
        return Ok(InstallResult {
            success: false,
            message: "CLI executable not found".to_string(),
        });
    }
    
    // Test CLI with -h flag
    let output = Command::new(&exe_path)
        .arg("-h")
        .output()
        .map_err(|e| format!("Failed to run CLI test: {}", e))?;
    
    if output.status.success() {
        Ok(InstallResult {
            success: true,
            message: "CLI installation verified successfully".to_string(),
        })
    } else {
        Ok(InstallResult {
            success: false,
            message: format!("CLI test failed: {}", String::from_utf8_lossy(&output.stderr)),
        })
    }
}

#[tauri::command]
async fn run_cli_command(install_path: String, args: Vec<String>) -> Result<CommandResult, String> {
    let exe_path = Path::new(&install_path).join("portablesource_main.exe");
    
    let output = Command::new(&exe_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run CLI command: {}", e))?;
    
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
    
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
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
    install_path: String,
    args: Vec<String>,
    event_id: String,
) -> Result<(), String> {
    let exe_path = Path::new(&install_path).join("portablesource_main.exe");
    
    let mut child = Command::new(&exe_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn CLI command: {}", e))?;
    
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
                    &format!("cli-output-{}", event_id_stdout),
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
                    &format!("cli-output-{}", event_id_stderr),
                    StreamOutput {
                        stream: "stderr".to_string(),
                        data: line,
                    },
                );
            }
        }
    });
    
    // Wait for the process to finish
    let status = child.wait().map_err(|e| format!("Failed to wait for CLI command: {}", e))?;
    
    // Wait for both tasks to complete
    let _ = tokio::join!(stdout_task, stderr_task);
    
    // Emit completion event
    let _ = app_handle.emit(
        &format!("cli-finished-{}", event_id),
        StreamFinished {
            success: status.success(),
            exit_code: status.code(),
        },
    );
    
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
    
    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
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
async fn check_environment_status(install_path: String) -> Result<EnvironmentStatus, String> {
    let exe_path = Path::new(&install_path).join("portablesource_main.exe");
    
    if !exe_path.exists() {
        return Ok(EnvironmentStatus {
            environment_exists: false,
            setup_completed: false,
            overall_status: "CLI not installed".to_string(),
        });
    }
    
    // Run CLI with --check-env flag from its directory with UTF-8 encoding
    let output = Command::new(&exe_path)
        .arg("--check-env")
        .current_dir(&install_path)
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .map_err(|e| format!("Failed to run CLI check-env: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(EnvironmentStatus {
            environment_exists: false,
            setup_completed: false,
            overall_status: format!("Environment check failed: {}", stderr),
        });
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Parse output to check for "Setup completed: YES" and environment status
    // Use text-based checks instead of emoji to avoid encoding issues
    let setup_completed = stdout.contains("Setup completed: YES");
    let environment_exists = stdout.contains("Environment exists:") && 
                           (stdout.contains("YES") || stdout.contains("✅") || stdout.contains("checkmark"));
    
    let overall_status = if setup_completed {
        "Ready".to_string()
    } else if environment_exists {
        "Environment exists but setup incomplete".to_string()
    } else {
        "Environment not found".to_string()
    };
    
    Ok(EnvironmentStatus {
        environment_exists,
        setup_completed,
        overall_status,
    })
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
            })
        }
        Err(_) => {
            Ok(InstallResult {
                success: true,
                message: "Registry key not found, nothing to clear".to_string(),
            })
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            app.handle().plugin(tauri_plugin_dialog::init())?;
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
            check_environment_installed,
            check_environment_status,
            check_repository_installed,
            list_directory_folders,
            run_command,
            run_command_stream,
            run_cli_command_stream,
            run_batch_in_new_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
