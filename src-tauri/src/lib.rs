pub mod commands;
pub mod core;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::EmuManager::default())
        .invoke_handler(tauri::generate_handler![
            commands::assemble,
            commands::run_program,
            commands::step,
            commands::reset,
            commands::stop,
            commands::set_breakpoints,
            commands::send_input,
            commands::read_mem,
            commands::read_framebuffer,
            commands::get_state,
            commands::export_elf,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
