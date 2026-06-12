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
            commands::push_key,
            commands::set_watchpoints,
            commands::write_reg,
            commands::write_mem,
            commands::read_mem,
            commands::read_framebuffer,
            commands::get_state,
            commands::export_elf,
            commands::leer_archivo_texto,
            commands::guardar_archivo_texto,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Re-export del stub GDB (RP6): disponible solo con la feature `gdb`,
/// para que el CLI (y los tests) usen la MISMA versión que la lib.
#[cfg(feature = "gdb")]
pub use udbserver;
