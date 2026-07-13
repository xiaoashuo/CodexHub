#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|arg| arg == "--router-only") {
        codex_router_shell_lib::run_router_only();
        return;
    }

    codex_router_shell_lib::run();
}
