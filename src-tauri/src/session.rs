use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

use crate::assessment::{Answer, Assessment, Step};
use crate::pdf::{civil_date, free_path};

pub const FORMAT: &str = "bellreader";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    pub format: String,
    pub version: u32,
    pub age_months: u32,
    pub saved_at: String,
    pub answers: Vec<Answer>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub age_months: u32,
    pub answered: usize,
    pub saved_at: String,
    pub finished: bool,
}

pub fn local_now() -> (i64, u32, u32, u32, u32) {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + 3_600;
    let (y, m, d) = civil_date(seconds);
    let day_seconds = seconds.rem_euclid(86_400);
    (y, m, d, (day_seconds / 3_600) as u32, (day_seconds % 3_600 / 60) as u32)
}

fn stamp() -> String {
    let (y, m, d, h, min) = local_now();
    format!("{y}-{m:02}-{d:02} {h:02}:{min:02}")
}

impl SaveFile {
    pub fn from_assessment(a: &Assessment) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            age_months: a.age_months(),
            saved_at: stamp(),
            answers: a.history().to_vec(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    pub fn parse(content: &str) -> Result<Self, String> {
        let file: SaveFile = serde_json::from_str(content)
            .map_err(|_| "Die Datei ist keine gültige .bellreader-Datei.".to_string())?;
        if file.format != FORMAT {
            return Err("Die Datei ist keine gültige .bellreader-Datei.".into());
        }
        if file.version > VERSION {
            return Err("Die Datei stammt aus einer neueren Bellreader-Version.".into());
        }
        Ok(file)
    }

    pub fn restore(&self) -> (Assessment, Step) {
        Assessment::replay(self.age_months, &self.answers)
    }

    pub fn info(&self) -> SessionInfo {
        let (assessment, step) = self.restore();
        SessionInfo {
            age_months: self.age_months,
            answered: assessment.history().len(),
            saved_at: self.saved_at.clone(),
            finished: matches!(step, Step::Finished { .. }),
        }
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("bellreader.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

pub fn run_worker(rx: Receiver<Vec<u8>>, path: &Path) {
    while let Ok(mut bytes) = rx.recv() {
        while let Ok(newer) = rx.try_recv() {
            bytes = newer;
        }
        let _ = write_atomic(path, &bytes);
    }
}

pub struct Autosaver {
    tx: Mutex<Sender<Vec<u8>>>,
    path: PathBuf,
}

impl Autosaver {
    pub fn start(path: PathBuf) -> Self {
        let (tx, rx) = channel();
        let worker_path = path.clone();
        thread::spawn(move || run_worker(rx, &worker_path));
        Self {
            tx: Mutex::new(tx),
            path,
        }
    }

    pub fn save(&self, assessment: &Assessment) {
        let bytes = SaveFile::from_assessment(assessment).to_bytes();
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(bytes);
        }
    }

    pub fn load(&self) -> Option<SaveFile> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|content| SaveFile::parse(&content).ok())
    }
}

pub fn autosave_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("autosave.bellreader"))
        .map_err(|e| e.to_string())
}

fn restore_into(
    file: &SaveFile,
    state: &State<'_, Mutex<Assessment>>,
    saver: &State<'_, Autosaver>,
) -> Result<Step, String> {
    let (assessment, step) = file.restore();
    let mut current = state.lock().map_err(|e| e.to_string())?;
    *current = assessment;
    saver.save(&current);
    Ok(step)
}

#[tauri::command]
pub fn autosave_info(saver: State<'_, Autosaver>) -> Option<SessionInfo> {
    saver.load().map(|file| file.info())
}

#[tauri::command]
pub fn resume_autosave(state: State<'_, Mutex<Assessment>>, saver: State<'_, Autosaver>) -> Result<Step, String> {
    let file = saver
        .load()
        .ok_or("Keine automatisch gespeicherte Beobachtung gefunden.")?;
    restore_into(&file, &state, &saver)
}

#[tauri::command]
pub fn load_session(
    content: String,
    state: State<'_, Mutex<Assessment>>,
    saver: State<'_, Autosaver>,
) -> Result<Step, String> {
    let file = SaveFile::parse(&content)?;
    restore_into(&file, &state, &saver)
}

#[tauri::command]
pub fn save_session(app: AppHandle, state: State<'_, Mutex<Assessment>>) -> Result<String, String> {
    let file = SaveFile::from_assessment(&*state.lock().map_err(|e| e.to_string())?);
    let (y, m, d, _, _) = local_now();
    let dir = app.path().download_dir().map_err(|e| e.to_string())?;
    let path = free_path(
        &dir,
        &format!("Bellreader_{y}-{m:02}-{d:02}_{}-Monate", file.age_months),
        "bellreader",
    );
    write_atomic(&path, &file.to_bytes()).map_err(|e| format!("Datei konnte nicht gespeichert werden: {e}"))?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers_for(age: u32, count: usize) -> Assessment {
        let mut a = Assessment::new(age);
        let mut step = a.advance();
        let mut n = 0;
        while let Step::Question { phase, .. } = step {
            if n == count {
                break;
            }
            n += 1;
            let answer = match phase {
                1..=5 => Answer::Ja,
                6 => Answer::Teilweise,
                7 => Answer::WeissNicht,
                _ => Answer::Nein,
            };
            step = a.answer(answer).unwrap();
        }
        a
    }

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bellreader-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn save_file_roundtrip() {
        let a = answers_for(40, 25);
        let file = SaveFile::from_assessment(&a);
        let text = String::from_utf8(file.to_bytes()).unwrap();
        assert!(text.contains("\"format\": \"bellreader\""));
        assert!(text.contains("\"weiss_nicht\"") || text.contains("\"ja\""));
        assert_eq!(SaveFile::parse(&text).unwrap(), file);
    }

    fn key(step: &Step) -> (usize, u32, usize, usize) {
        match step {
            Step::Question { area_index, phase, condition_index, answered, .. } => {
                (*area_index, *phase, *condition_index, *answered)
            }
            Step::Finished { .. } => panic!("expected a question"),
        }
    }

    #[test]
    fn restore_continues_at_the_same_question() {
        let mut a = answers_for(40, 24);
        let file = SaveFile::from_assessment(&a);
        let (_, restored) = file.restore();
        assert_eq!(key(&restored), key(&a.advance()));
        assert_eq!(key(&restored).3, 24);
    }

    #[test]
    fn finished_file_opens_on_results_and_can_go_back() {
        let a = answers_for(40, usize::MAX);
        let file = SaveFile::from_assessment(&a);
        let (mut restored, step) = file.restore();
        assert!(matches!(step, Step::Finished { .. }));
        assert!(file.info().finished);
        assert_eq!(file.info().answered, a.history().len());
        assert!(matches!(restored.back(), Step::Question { .. }));
    }

    #[test]
    fn rejects_wrong_files() {
        assert!(SaveFile::parse("not json").is_err());
        assert!(SaveFile::parse(r#"{"format":"other","version":1,"age_months":4,"saved_at":"","answers":[]}"#).is_err());
        assert!(SaveFile::parse(r#"{"format":"bellreader","version":99,"age_months":4,"saved_at":"","answers":[]}"#).is_err());
        assert!(SaveFile::parse(r#"{"format":"bellreader","version":1,"age_months":4,"saved_at":"","answers":["vielleicht"]}"#).is_err());
    }

    #[test]
    fn worker_writes_the_latest_state() {
        let path = temp_file("worker.bellreader");
        let (tx, rx) = channel();
        for count in [3, 10, 17] {
            tx.send(SaveFile::from_assessment(&answers_for(40, count)).to_bytes()).unwrap();
        }
        drop(tx);
        run_worker(rx, &path);
        let saved = SaveFile::parse(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.answers.len(), 17);
        assert!(!path.with_extension("bellreader.tmp").exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn autosaver_saves_in_the_background() {
        let path = temp_file("autosave.bellreader");
        let saver = Autosaver::start(path.clone());
        let a = answers_for(19, 12);
        saver.save(&a);
        let mut loaded = None;
        for _ in 0..200 {
            loaded = saver.load();
            if loaded.is_some() {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
        let loaded = loaded.expect("autosave file was not written");
        assert_eq!(loaded.age_months, 19);
        assert_eq!(loaded.answers.len(), 12);
        let _ = fs::remove_file(path);
    }
}
