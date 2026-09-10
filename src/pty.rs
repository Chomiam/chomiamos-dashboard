use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem, MasterPty};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

pub struct PtyManager {
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            master: Arc::new(Mutex::new(None)),
            writer: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&self, app: AppHandle, program: String, args: Vec<String>, cols: u16, rows: u16) -> Result<(), String> {
        {
            let mut w = self.writer.lock().unwrap();
            *w = None;
            let mut m = self.master.lock().unwrap();
            *m = None;
        }

        let pty_system = NativePtySystem::default();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| format!("Erreur ouverture PTY : {}", e))?;

        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);

        let mut child = pair.slave.spawn_command(cmd).map_err(|e| format!("Erreur lancement processus : {}", e))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| format!("Erreur clone reader : {}", e))?;
        let writer = pair.master.take_writer().map_err(|e| format!("Erreur take writer : {}", e))?;

        {
            let mut w_lock = self.writer.lock().unwrap();
            *w_lock = Some(writer);
            let mut m_lock = self.master.lock().unwrap();
            *m_lock = Some(pair.master);
        }

        let writer_ref = self.writer.clone();
        let master_ref = self.master.clone();

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = app.emit("pty-data", chunk);
            }

            let status = child.wait();
            let exit_code = match status {
                Ok(s) => s.exit_code() as i32,
                Err(_) => 1,
            };

            let _ = app.emit("pty-exit", exit_code);

            let mut w_lock = writer_ref.lock().unwrap();
            *w_lock = None;
            let mut m_lock = master_ref.lock().unwrap();
            *m_lock = None;
        });

        Ok(())
    }

    pub fn write(&self, data: &str) -> Result<(), String> {
        let mut lock = self.writer.lock().unwrap();
        if let Some(w) = lock.as_mut() {
            w.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
            w.flush().map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Aucune session PTY active".into())
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        let lock = self.master.lock().unwrap();
        if let Some(m) = lock.as_ref() {
            m.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
