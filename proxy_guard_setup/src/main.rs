#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result};
use proxy_guard_core::{
    AppConfig, CleanupScope, ManagedRule, ProxyCandidate, RegistryProxySettingsStore,
    config_path, is_helper_auto_start_enabled, is_portable_mode, load_config, save_config,
    scan_candidates_from_store, selected_rules_from_candidates, set_helper_auto_start,
};
use slint::{Model, ModelRc, SharedString, Timer, VecModel};

slint::include_modules!();

fn main() -> Result<()> {
    let window = AppWindow::new().context("failed to create setup window")?;
    let config = load_config().context("failed to load config")?;
    let helper_auto_start = is_helper_auto_start_enabled().unwrap_or(config.auto_start_helper);

    let model = Rc::new(VecModel::from(Vec::<CandidateRow>::new()));
    window.set_candidates(ModelRc::from(model.clone()));
    let scope_model = Rc::new(VecModel::from(
        CleanupScope::options()
            .iter()
            .map(|scope| SharedString::from(scope.display_name()))
            .collect::<Vec<_>>(),
    ));
    window.set_cleanup_scopes(ModelRc::from(scope_model));
    window.set_selected_scope_index(scope_to_index(config.cleanup_scope) as i32);
    window.set_cleanup_on_login(config.cleanup_on_login);
    window.set_auto_start_helper(helper_auto_start);
    window.set_status_text(initial_loading_status_text().into());

    let shared_candidates = Rc::new(RefCell::new(Vec::<ProxyCandidate>::new()));
    let ui_model = model.clone();
    window.on_toggle_candidate(move |index, selected| {
        let row_index = index as usize;
        if let Some(mut row) = ui_model.row_data(row_index) {
            row.selected = selected;
            ui_model.set_row_data(row_index, row);
        }
    });

    let existing_config = Rc::new(RefCell::new(config));

    let initial_scan_window = window.as_weak();
    let initial_scan_model = model.clone();
    let initial_scan_store = RegistryProxySettingsStore::new();
    let initial_scan_config = existing_config.clone();
    let initial_scan_candidates = shared_candidates.clone();
    Timer::single_shot(Duration::from_millis(0), move || {
        let config = initial_scan_config.borrow().clone();
        match scan_candidates_from_store(&initial_scan_store) {
            Ok(candidates) => {
                *initial_scan_candidates.borrow_mut() = candidates.clone();
                initial_scan_model.set_vec(build_rows(&candidates, &config));
                if let Some(window) = initial_scan_window.upgrade() {
                    window.set_status_text(initial_status_text(&candidates).into());
                }
            }
            Err(error) => {
                if let Some(window) = initial_scan_window.upgrade() {
                    window.set_status_text(format!("初始化扫描失败：{error:#}").into());
                }
            }
        }
    });

    let rescan_window = window.as_weak();
    let rescan_model = model.clone();
    let rescan_store = RegistryProxySettingsStore::new();
    let existing_config_for_rescan = existing_config.clone();
    let shared_candidates_for_rescan = shared_candidates.clone();
    window.on_rescan(move || {
        let config = existing_config_for_rescan.borrow().clone();
        match scan_candidates_from_store(&rescan_store) {
            Ok(candidates) => {
                *shared_candidates_for_rescan.borrow_mut() = candidates.clone();
                rescan_model.set_vec(build_rows(&candidates, &config));
                if let Some(window) = rescan_window.upgrade() {
                    window.set_status_text(initial_status_text(&candidates).into());
                }
            }
            Err(error) => {
                if let Some(window) = rescan_window.upgrade() {
                    window.set_status_text(format!("重新扫描失败：{error:#}").into());
                }
            }
        }
    });

    let save_window = window.as_weak();
    let save_model = model.clone();
    let save_candidates = shared_candidates.clone();
    let saved_config_state = existing_config.clone();
    window.on_save(move || {
        let selected_ids = selected_ids_from_model(&save_model);
        let managed_rules =
            selected_rules_from_candidates(&save_candidates.borrow(), &selected_ids);
        let mut config = saved_config_state.borrow().clone();
        config.managed_rules = managed_rules;
        config.cleanup_scope = index_to_scope(
            save_window
                .upgrade()
                .map(|window| window.get_selected_scope_index())
                .unwrap_or_default() as usize,
        );
        config.cleanup_on_login = save_window
            .upgrade()
            .map(|window| window.get_cleanup_on_login())
            .unwrap_or(false);
        config.auto_start_helper = save_window
            .upgrade()
            .map(|window| window.get_auto_start_helper())
            .unwrap_or(false);

        match set_helper_auto_start(config.auto_start_helper)
            .and_then(|_| save_config(config.clone()))
        {
            Ok(()) => {
                *saved_config_state.borrow_mut() = config;
                if let Some(window) = save_window.upgrade() {
                    let path = config_path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|_| "%LOCALAPPDATA%\\ProxyGuard\\config.json".to_string());
                    window.set_status_text(format!("已保存配置：{path}").into());
                }
            }
            Err(error) => {
                if let Some(window) = save_window.upgrade() {
                    window.set_status_text(format!("保存失败：{error:#}").into());
                }
            }
        }
    });

    window.run().context("setup UI exited unexpectedly")?;
    Ok(())
}

fn build_rows(candidates: &[ProxyCandidate], config: &AppConfig) -> Vec<CandidateRow> {
    candidates
        .iter()
        .map(|candidate| CandidateRow {
            id: candidate.id.clone().into(),
            title: candidate.title.clone().into(),
            detail: candidate.detail.clone().into(),
            recommended: candidate.recommended,
            selected: is_rule_selected(&candidate.rule, &config.managed_rules),
        })
        .collect()
}

fn is_rule_selected(rule: &ManagedRule, selected_rules: &[ManagedRule]) -> bool {
    selected_rules.iter().any(|candidate| candidate == rule)
}

fn selected_ids_from_model(model: &Rc<VecModel<CandidateRow>>) -> BTreeSet<String> {
    (0..model.row_count())
        .filter_map(|index| model.row_data(index))
        .filter(|row| row.selected)
        .map(|row| row.id.to_string())
        .collect()
}

fn scope_to_index(scope: CleanupScope) -> usize {
    match scope {
        CleanupScope::ShutdownAndRestart | CleanupScope::ShutdownOnly => 0,
        CleanupScope::ShutdownRestartAndLogoff => 1,
    }
}

fn index_to_scope(index: usize) -> CleanupScope {
    match index {
        1 => CleanupScope::ShutdownRestartAndLogoff,
        _ => CleanupScope::ShutdownAndRestart,
    }
}

fn initial_status_text(candidates: &[ProxyCandidate]) -> String {
    let mode = if is_portable_mode().unwrap_or(false) {
        "当前模式：便携模式，配置保存在程序目录旁的 config 文件夹。"
    } else {
        "当前模式：安装模式，配置保存在 %LOCALAPPDATA%\\ProxyGuard。"
    };

    if candidates.is_empty() {
        format!("{mode}\n当前没有检测到系统代理候选项。")
    } else {
        format!(
            "{mode}\n已检测到 {} 个候选项。推荐优先勾选回环地址代理。",
            candidates.len()
        )
    }
}

fn initial_loading_status_text() -> String {
    let mode = if is_portable_mode().unwrap_or(false) {
        "当前模式：便携模式，配置保存在程序目录旁的 config 文件夹。"
    } else {
        "当前模式：安装模式，配置保存在 %LOCALAPPDATA%\\ProxyGuard。"
    };

    format!("{mode}\n正在读取当前系统代理，请稍候…")
}
