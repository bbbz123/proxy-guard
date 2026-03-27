#![windows_subsystem = "windows"]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use proxy_guard_core::{CleanupEvent, RegistryProxySettingsStore, cleanup_from_store, load_config};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, ENDSESSION_LOGOFF,
    GetMessageW, HMENU, MSG, PostQuitMessage, RegisterClassW, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CREATE, WM_DESTROY, WM_ENDSESSION, WM_NCCREATE,
    WM_QUERYENDSESSION, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

const WINDOW_CLASS_NAME: PCWSTR = w!("ProxyGuardHelperWindow");
const MUTEX_NAME: PCWSTR = w!("Local\\ProxyGuardHelperMutex");

static CLEANUP_TRIGGERED: AtomicBool = AtomicBool::new(false);

fn main() -> Result<()> {
    let _mutex = acquire_single_instance_mutex().context("failed to acquire single-instance mutex")?;
    run_login_cleanup_if_enabled().context("failed during optional login cleanup")?;
    let instance = unsafe { GetModuleHandleW(None) }.context("failed to get module handle")?;
    let class_atom = register_window_class(instance.into())?;
    if class_atom == 0 {
        return Err(anyhow::anyhow!("failed to register helper window class"));
    }

    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            WINDOW_CLASS_NAME,
            w!("Proxy Guard Helper"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND::default()),
            Some(HMENU::default()),
            Some(instance.into()),
            Some(std::ptr::null_mut::<c_void>()),
        )
    }
    .context("failed to create hidden helper window")?;

    let _ = window;

    run_message_loop()?;
    Ok(())
}

fn run_login_cleanup_if_enabled() -> Result<()> {
    let config = load_config().context("failed to load proxy guard config")?;
    if !config.cleanup_on_login {
        return Ok(());
    }

    let store = RegistryProxySettingsStore::new();
    let event = CleanupEvent { is_logoff: false };
    let _ = cleanup_from_store(&store, &config, &event)?;
    Ok(())
}

fn acquire_single_instance_mutex() -> Result<OwnedMutex> {
    let handle = unsafe { CreateMutexW(None, false, MUTEX_NAME) }.context("CreateMutexW failed")?;
    let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if already_exists {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err(anyhow::anyhow!("Proxy Guard helper is already running"));
    }
    Ok(OwnedMutex(handle))
}

fn register_window_class(instance: HINSTANCE) -> Result<u16> {
    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        hInstance: instance,
        lpszClassName: WINDOW_CLASS_NAME,
        lpfnWndProc: Some(window_proc),
        ..Default::default()
    };

    let class_atom = unsafe { RegisterClassW(&window_class) };
    Ok(class_atom)
}

fn run_message_loop() -> Result<()> {
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.into() {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCCREATE => {
            LRESULT(1)
        }
        WM_CREATE => LRESULT(0),
        WM_QUERYENDSESSION => LRESULT(1),
        WM_ENDSESSION => {
            if wparam == WPARAM(usize::from(true)) {
                let is_logoff = (lparam.0 & ENDSESSION_LOGOFF as isize) != 0;
                let _ = unsafe { maybe_cleanup(window, is_logoff) };
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

unsafe fn maybe_cleanup(window: HWND, is_logoff: bool) -> Result<()> {
    if CLEANUP_TRIGGERED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let _ = window;
    let config = load_config().context("failed to reload proxy guard config")?;
    let store = RegistryProxySettingsStore::new();
    let event = CleanupEvent { is_logoff };
    let _ = cleanup_from_store(&store, &config, &event)?;
    Ok(())
}

struct OwnedMutex(HANDLE);

impl Drop for OwnedMutex {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}
