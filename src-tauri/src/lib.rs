mod assessment;
mod pdf;
mod session;
mod table;

use tauri::Manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(std::sync::Mutex::new(assessment::Assessment::default()))
        .setup(|app| {
            let path = session::autosave_path(app.handle())?;
            app.manage(session::Autosaver::start(path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            table::get_areas,
            table::get_phase,
            table::get_table,
            assessment::start_assessment,
            assessment::answer,
            assessment::go_back,
            pdf::export_pdf,
            session::autosave_info,
            session::resume_autosave,
            session::load_session,
            session::save_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
