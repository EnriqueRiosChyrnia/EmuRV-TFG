pub mod commands;
pub mod core;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::EmuManager::default());

    #[cfg(debug_assertions)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::assemble,
        commands::run_program,
        commands::step,
        commands::reset,
        commands::stop,
        commands::set_breakpoints,
        commands::send_input,
        commands::read_mem,
        commands::get_state,
        commands::export_elf,
        commands::autotest_report,
    ]);
    #[cfg(not(debug_assertions))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::assemble,
        commands::run_program,
        commands::step,
        commands::reset,
        commands::stop,
        commands::set_breakpoints,
        commands::send_input,
        commands::read_mem,
        commands::get_state,
        commands::export_elf,
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
