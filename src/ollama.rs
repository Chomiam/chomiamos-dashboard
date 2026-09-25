use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OllamaInstalledModel {
    pub name: String,
    pub model: String,
    pub size: u64,
    pub modified_at: Option<String>,
    pub digest: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
    pub format: Option<String>,
    pub family: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OllamaLoadedModel {
    pub name: String,
    pub model: String,
    pub size: u64,
    pub size_vram: u64,
    pub expires_at: Option<String>,
    pub context_length: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OllamaStatus {
    pub online: bool,
    pub port: u16,
    pub version: Option<String>,
    pub service_active: bool,
    pub installed_models: Vec<OllamaInstalledModel>,
    pub loaded_models: Vec<OllamaLoadedModel>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OllamaPullProgressEvent {
    pub model: String,
    pub status: String,
    pub digest: Option<String>,
    pub total: Option<u64>,
    pub completed: Option<u64>,
    pub percentage: Option<f32>,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct OllamaManager {
    running_pulls: Arc<Mutex<HashMap<String, u32>>>,
}

impl OllamaManager {
    pub fn new() -> Self {
        Self {
            running_pulls: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register_pull(&self, model: &str, pid: u32) {
        if let Ok(mut lock) = self.running_pulls.lock() {
            lock.insert(model.to_string(), pid);
        }
    }

    pub fn unregister_pull(&self, model: &str) {
        if let Ok(mut lock) = self.running_pulls.lock() {
            lock.remove(model);
        }
    }

    pub fn cancel_pull(&self, model: &str) -> bool {
        if let Ok(mut lock) = self.running_pulls.lock() {
            if let Some(pid) = lock.remove(model) {
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                return true;
            }
        }
        false
    }
}

#[tauri::command]
pub fn get_ollama_status(port: Option<u16>) -> Result<OllamaStatus, String> {
    let port = port.unwrap_or(11434);

    // Vérifie si le service systemd est actif
    let service_active = Command::new("systemctl")
        .args(["is-active", "ollama"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false);

    // Test de connectivité vers l'API Ollama
    let ver_url = format!("http://127.0.0.1:{}/api/version", port);
    let ver_res = Command::new("curl")
        .args(["-s", "--connect-timeout", "2", "-m", "4", &ver_url])
        .output();

    let (online, version) = match ver_res {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                let ver = v.get("version").and_then(|val| val.as_str()).map(|x| x.to_string());
                (true, ver)
            } else {
                (true, None)
            }
        }
        _ => (false, None),
    };

    if !online {
        let err_msg = if service_active {
            format!("Le service systemd 'ollama' est actif mais ne répond pas sur http://127.0.0.1:{}", port)
        } else {
            format!("Le service Ollama est arrêté ou inaccessible sur le port {}", port)
        };
        return Ok(OllamaStatus {
            online: false,
            port,
            version: None,
            service_active,
            installed_models: Vec::new(),
            loaded_models: Vec::new(),
            error: Some(err_msg),
        });
    }

    // Récupération des modèles installés (/api/tags)
    let tags_url = format!("http://127.0.0.1:{}/api/tags", port);
    let mut installed_models = Vec::new();
    if let Ok(out) = Command::new("curl")
        .args(["-s", "--connect-timeout", "2", "-m", "5", &tags_url])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(arr) = val.get("models").and_then(|m| m.as_array()) {
                for m in arr {
                    let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let model = m.get("model").and_then(|v| v.as_str()).unwrap_or(&name).to_string();
                    let size = m.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    let modified_at = m.get("modified_at").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let digest = m.get("digest").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let details = m.get("details");
                    let parameter_size = details.and_then(|d| d.get("parameter_size")).and_then(|v| v.as_str()).map(|s| s.to_string());
                    let quantization_level = details.and_then(|d| d.get("quantization_level")).and_then(|v| v.as_str()).map(|s| s.to_string());
                    let format = details.and_then(|d| d.get("format")).and_then(|v| v.as_str()).map(|s| s.to_string());
                    let family = details.and_then(|d| d.get("family")).and_then(|v| v.as_str()).map(|s| s.to_string());

                    installed_models.push(OllamaInstalledModel {
                        name,
                        model,
                        size,
                        modified_at,
                        digest,
                        parameter_size,
                        quantization_level,
                        format,
                        family,
                    });
                }
            }
        }
    }

    // Récupération des modèles actuellement chargés en mémoire (/api/ps)
    let ps_url = format!("http://127.0.0.1:{}/api/ps", port);
    let mut loaded_models = Vec::new();
    if let Ok(out) = Command::new("curl")
        .args(["-s", "--connect-timeout", "2", "-m", "5", &ps_url])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(arr) = val.get("models").and_then(|m| m.as_array()) {
                for m in arr {
                    let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let model = m.get("model").and_then(|v| v.as_str()).unwrap_or(&name).to_string();
                    let size = m.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    let size_vram = m.get("size_vram").and_then(|v| v.as_u64()).unwrap_or(0);
                    let expires_at = m.get("expires_at").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let context_length = m.get("context_length").and_then(|v| v.as_u64());

                    loaded_models.push(OllamaLoadedModel {
                        name,
                        model,
                        size,
                        size_vram,
                        expires_at,
                        context_length,
                    });
                }
            }
        }
    }

    Ok(OllamaStatus {
        online: true,
        port,
        version,
        service_active,
        installed_models,
        loaded_models,
        error: None,
    })
}

#[tauri::command]
pub fn load_ollama_model(model: String, port: Option<u16>) -> Result<String, String> {
    let port = port.unwrap_or(11434);
    let url = format!("http://127.0.0.1:{}/api/generate", port);
    let payload = serde_json::json!({
        "model": model,
        "prompt": "",
        "keep_alive": "10m"
    });
    let body = serde_json::to_string(&payload).unwrap();

    let out = Command::new("curl")
        .args(["-s", "-X", "POST", &url, "-H", "Content-Type: application/json", "-d", &body])
        .output()
        .map_err(|e| format!("Erreur exécution curl: {}", e))?;

    let text = String::from_utf8_lossy(&out.stdout);
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(err) = val.get("error").and_then(|v| v.as_str()) {
            return Err(format!("Ollama: {}", err));
        }
    }

    Ok(format!("Modèle '{}' préchargé en mémoire vive / VRAM", model))
}

#[tauri::command]
pub fn unload_ollama_model(model: String, port: Option<u16>) -> Result<String, String> {
    let port = port.unwrap_or(11434);
    let url = format!("http://127.0.0.1:{}/api/generate", port);
    let payload = serde_json::json!({
        "model": model,
        "prompt": "",
        "keep_alive": 0
    });
    let body = serde_json::to_string(&payload).unwrap();

    let out = Command::new("curl")
        .args(["-s", "-X", "POST", &url, "-H", "Content-Type: application/json", "-d", &body])
        .output()
        .map_err(|e| format!("Erreur exécution curl: {}", e))?;

    let text = String::from_utf8_lossy(&out.stdout);
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(err) = val.get("error").and_then(|v| v.as_str()) {
            return Err(format!("Ollama: {}", err));
        }
    }

    Ok(format!("Modèle '{}' déchargé de la mémoire (VRAM libérée)", model))
}

#[tauri::command]
pub fn pull_ollama_model(
    app: AppHandle,
    manager: State<'_, OllamaManager>,
    model: String,
    port: Option<u16>,
) -> Result<(), String> {
    let port = port.unwrap_or(11434);
    let pull_url = format!("http://127.0.0.1:{}/api/pull", port);
    let body = serde_json::json!({
        "name": model,
        "stream": true
    }).to_string();

    let model_clone = model.clone();
    let app_clone = app.clone();
    let manager_ref = manager.inner().clone();

    thread::spawn(move || {
        let mut child = match Command::new("curl")
            .args([
                "-N",
                "-s",
                "-X", "POST",
                &pull_url,
                "-H", "Content-Type: application/json",
                "-d", &body,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = app_clone.emit(
                    "ollama-pull-progress",
                    OllamaPullProgressEvent {
                        model: model_clone.clone(),
                        status: "error".to_string(),
                        digest: None,
                        total: None,
                        completed: None,
                        percentage: None,
                        done: true,
                        error: Some(format!("Impossible de lancer curl: {}", e)),
                    },
                );
                return;
            }
        };

        let pid = child.id();
        manager_ref.register_pull(&model_clone, pid);

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line_str) => {
                        let line_str = line_str.trim();
                        if line_str.is_empty() {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line_str) {
                            if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                                let _ = app_clone.emit(
                                    "ollama-pull-progress",
                                    OllamaPullProgressEvent {
                                        model: model_clone.clone(),
                                        status: "error".to_string(),
                                        digest: None,
                                        total: None,
                                        completed: None,
                                        percentage: None,
                                        done: true,
                                        error: Some(err.to_string()),
                                    },
                                );
                                manager_ref.unregister_pull(&model_clone);
                                return;
                            }

                            let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string();
                            let digest = v.get("digest").and_then(|s| s.as_str()).map(|s| s.to_string());
                            let total = v.get("total").and_then(|t| t.as_u64());
                            let completed = v.get("completed").and_then(|c| c.as_u64());
                            let percentage = match (completed, total) {
                                (Some(c), Some(t)) if t > 0 => Some(((c as f64 / t as f64) * 100.0) as f32),
                                _ => None,
                            };

                            let is_success = status == "success";
                            let _ = app_clone.emit(
                                "ollama-pull-progress",
                                OllamaPullProgressEvent {
                                    model: model_clone.clone(),
                                    status: status.clone(),
                                    digest,
                                    total,
                                    completed,
                                    percentage: if is_success { Some(100.0) } else { percentage },
                                    done: is_success,
                                    error: None,
                                },
                            );

                            if is_success {
                                manager_ref.unregister_pull(&model_clone);
                                let _ = child.wait();
                                return;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }

        let status = child.wait();
        manager_ref.unregister_pull(&model_clone);

        if let Ok(exit_status) = status {
            if !exit_status.success() {
                let _ = app_clone.emit(
                    "ollama-pull-progress",
                    OllamaPullProgressEvent {
                        model: model_clone.clone(),
                        status: "terminated".to_string(),
                        digest: None,
                        total: None,
                        completed: None,
                        percentage: None,
                        done: true,
                        error: Some(format!("Le processus curl s'est terminé avec le code {:?}", exit_status.code())),
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_ollama_pull(manager: State<'_, OllamaManager>, model: String) -> Result<bool, String> {
    Ok(manager.cancel_pull(&model))
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_manager_register_and_cancel() {
        let mgr = OllamaManager::new();
        mgr.register_pull("test-model", 9999999);
        assert!(mgr.running_pulls.lock().unwrap().contains_key("test-model"));
        
        // Cancel should remove it (kill will error on nonexistent pid, which is fine)
        let _ = mgr.cancel_pull("test-model");
        assert!(!mgr.running_pulls.lock().unwrap().contains_key("test-model"));
    }

    #[test]
    fn test_parse_tags_json() {
        let sample = r#"{
            "models": [
                {
                    "name": "qwen2.5-coder:7b",
                    "model": "qwen2.5-coder:7b",
                    "modified_at": "2026-09-25T09:25:36Z",
                    "size": 4683087561,
                    "digest": "dae161e27b0e",
                    "details": {
                        "parameter_size": "7.6B",
                        "quantization_level": "Q4_K_M",
                        "format": "gguf",
                        "family": "qwen2"
                    }
                }
            ]
        }"#;

        let val: serde_json::Value = serde_json::from_str(sample).unwrap();
        let arr = val.get("models").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let m = &arr[0];
        assert_eq!(m.get("name").unwrap().as_str().unwrap(), "qwen2.5-coder:7b");
        assert_eq!(m.get("size").unwrap().as_u64().unwrap(), 4683087561);
        let details = m.get("details").unwrap();
        assert_eq!(details.get("parameter_size").unwrap().as_str().unwrap(), "7.6B");
    }

    #[test]
    fn test_parse_ps_json() {
        let sample = r#"{
            "models": [
                {
                    "name": "qwen2.5-coder:7b",
                    "model": "qwen2.5-coder:7b",
                    "size": 4748056984,
                    "size_vram": 4748056984,
                    "expires_at": "2026-09-25T09:37:14Z",
                    "context_length": 4096
                }
            ]
        }"#;

        let val: serde_json::Value = serde_json::from_str(sample).unwrap();
        let arr = val.get("models").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let m = &arr[0];
        assert_eq!(m.get("size_vram").unwrap().as_u64().unwrap(), 4748056984);
        assert_eq!(m.get("context_length").unwrap().as_u64().unwrap(), 4096);
    }

    #[test]
    fn test_get_ollama_status_offline_port() {
        // Port 59999 is unlikely to have ollama listening
        let res = get_ollama_status(Some(59999));
        assert!(res.is_ok());
        let status = res.unwrap();
        assert!(!status.online);
        assert_eq!(status.port, 59999);
        assert!(status.installed_models.is_empty());
        assert!(status.loaded_models.is_empty());
        assert!(status.error.is_some());
    }
}
