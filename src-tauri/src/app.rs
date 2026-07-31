use super::*;

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, TRAY_SHOW_ID, "显示窗口", true, None::<&str>)?;
    let exit_item = MenuItem::with_id(app, TRAY_EXIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &exit_item])?;

    let mut tray_builder = TrayIconBuilder::new()
        .tooltip("Codex 伴侣")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_SHOW_ID => show_main_window(app),
            TRAY_EXIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder.build(app)?;

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn run_router_only() {
    match start_router_blocking() {
        Ok(result) => {
            println!("Router started: {}", result.health_url);
        }
        Err(error) => {
            eprintln!("Router start failed: {}", error);
            std::process::exit(1);
        }
    }

    loop {
        thread::park();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            setup_tray(app)?;
            if let Err(error) = read_app_settings() {
                eprintln!("App settings startup validation error: {}", error);
            }
            if let Err(error) = recover_router_state_on_startup() {
                eprintln!("Router startup recovery error: {}", error);
            }
            if let Err(error) = ensure_codex_oauth_callback_listener() {
                eprintln!("OAuth callback listener startup error: {}", error);
            }
            ensure_account_usage_refresh_worker();
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_router,
            stop_router,
            router_status,
            restart_router,
            check_router_port_occupancy,
            router_request_logs,
            clear_router_request_logs,
            account_proxy_request_logs,
            token_usage_summary,
            dashboard_quick_counts,
            clear_account_proxy_request_logs,
            append_app_log,
            search_app_logs,
            clear_app_logs,
            app_log_file_info,
            clean_maintenance_data,
            create_migration_backup,
            inspect_migration_backup,
            import_migration_backup,
            check_latest_version,
            read_provider_config,
            write_provider_config,
            export_provider_config,
            import_provider_config,
            fetch_provider_models,
            test_provider_model,
            test_provider_model_chat,
            test_proxy_connection,
            detect_proxy_connection,
            preview_local_file,
            read_app_settings,
            detect_codex_exe_path_for_settings,
            scan_codex_accounts,
            switch_codex_account,
            remove_codex_account_snapshot,
            update_codex_account_expiration,
            export_codex_accounts,
            refresh_codex_accounts_usage,
            refresh_codex_account_usage,
            refresh_codex_account_token,
            start_codex_account_login,
            codex_oauth_login_status,
            codex_oauth_callback_listener_status,
            import_current_codex_account,
            import_chatgpt_session_account,
            import_cpa_account,
            start_codex_client_login,
            restart_codex_app,
            open_external_url,
            download_and_install_update,
            cancel_update_download,
            local_config_paths,
            load_mcp_servers,
            upsert_mcp_server,
            set_mcp_server_enabled,
            remove_mcp_server,
            load_installed_skills,
            load_codex_plugins,
            set_codex_plugin_enabled,
            set_codex_plugin_skill_enabled,
            load_skill_backups,
            import_skill,
            remove_skill,
            restore_skill_backup,
            delete_skill_backup,
            quick_codex_thread_summary,
            scan_codex_threads,
            check_restore_codex_thread_index,
            restore_codex_thread_index,
            delete_codex_thread_files,
            toggle_codex_token_auto_renew,
            write_app_settings,
            sync_enabled_models_to_catalog,
            ensure_required_config_files,
            prepare_router_startup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
