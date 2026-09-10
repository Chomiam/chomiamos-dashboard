#![allow(dead_code)]
use serde::Serialize;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_stream::wrappers::LinesStream;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheck {
    pub updates_available: bool,
    pub current_commit: String,
    pub message: String,
}

pub async fn check_updates() -> UpdateCheck {
    // Check git status in /etc/nixos
    let git_check = Command::new("git")
        .args(["-C", "/etc/nixos", "rev-parse", "--short", "HEAD"])
        .output()
        .await;

    let commit = match git_check {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "local".to_string(),
    };

    UpdateCheck {
        updates_available: false,
        current_commit: commit,
        message: "Système synchronisé avec /etc/nixos".to_string(),
    }
}

pub struct ProcessOutputLine {
    pub text: String,
    pub is_error: bool,
}

pub async fn spawn_command_stream(
    action: &str,
) -> Result<LinesStream<BufReader<tokio::io::DuplexStream>>, String> {
    let (mut client_writer, client_reader) = tokio::io::duplex(64 * 1024);

    let (program, args): (&str, Vec<&str>) = match action {
        "switch" => ("nh", vec!["os", "switch", "/etc/nixos"]),
        "switch-update" => ("nh", vec!["os", "switch", "-u", "/etc/nixos"]),
        "boot" => ("nh", vec!["os", "boot", "/etc/nixos"]),
        "boot-update" => ("nh", vec!["os", "boot", "-u", "/etc/nixos"]),
        "clean-generations" => ("nh", vec!["clean", "all", "--keep", "3"]),
        "clean-all" => ("nh", vec!["clean", "all"]),
        "optimise" => ("nix", vec!["store", "optimise"]),
        _ => return Err(format!("Action inconnue: {}", action)),
    };

    let mut cmd = Command::new(program);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Impossible de lancer {}: {}", program, e))?;

    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let mut out_reader = BufReader::new(stdout).lines();
        let mut err_reader = BufReader::new(stderr).lines();

        let banner = format!("🚀 Démarrage de : {} {}\n", program, args.join(" "));
        let _ = client_writer.write_all(banner.as_bytes()).await;

        loop {
            tokio::select! {
                line = out_reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            let _ = client_writer.write_all(format!("{}\n", l).as_bytes()).await;
                        }
                        _ => break,
                    }
                }
                line = err_reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            let _ = client_writer.write_all(format!("{}\n", l).as_bytes()).await;
                        }
                        _ => break,
                    }
                }
            }
        }

        let status = child.wait().await;
        match status {
            Ok(s) if s.success() => {
                let _ = client_writer.write_all("\n\x1b[32m✔ Opération terminée avec succès !\x1b[0m\n".as_bytes()).await;
            }
            Ok(s) => {
                let _ = client_writer.write_all(format!("\n\x1b[31m✘ L'opération a échoué avec le code {}\x1b[0m\n", s.code().unwrap_or(1)).as_bytes()).await;
            }
            Err(e) => {
                let _ = client_writer.write_all(format!("\n\x1b[31m✘ Erreur d'exécution: {}\x1b[0m\n", e).as_bytes()).await;
            }
        }
    });

    let reader = BufReader::new(client_reader);
    Ok(LinesStream::new(reader.lines()))
}
