use std::os::windows::process::CommandExt;
use std::process::Command;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{HRESULT, PCWSTR};

pub const RESET_ARG: &str = "--reset-bluetooth";

const NO_WINDOW: u32 = 0x0800_0000;
const ERROR_CANCELLED: u32 = 1223;
const SERVICES: [&str; 2] = ["BTAGService", "Audiosrv"];
const SETTLE: Duration = Duration::from_secs(2);
const OFF_TIME: Duration = Duration::from_secs(5);

const EXIT_DONE: u32 = 0;
const EXIT_NO_ADAPTER: u32 = 2;
const EXIT_IN_USE: u32 = 3;
const EXIT_NOT_BACK: u32 = 4;

pub enum Outcome {
    Done,
    Cancelled,
    NoAdapter,
    InUse,
    NotBack,
    Failed(String),
}

fn system_tool(name: &str) -> Command {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    let mut command = Command::new(format!("{root}\\System32\\{name}"));
    command.creation_flags(NO_WINDOW);
    command
}

fn stop_service(name: &str) {
    let _ = system_tool("net.exe").args(["stop", name, "/y"]).status();
}

fn start_service(name: &str) {
    let _ = system_tool("net.exe").args(["start", name]).status();
}

fn adapter_id() -> Option<String> {
    let output = system_tool("pnputil.exe").args(["/enum-devices", "/class", "Bluetooth"]).output().ok()?;
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .find(|token| token.to_ascii_uppercase().starts_with("USB\\VID_"))
        .map(str::to_string)
}

fn pnputil_ok(flag: &str, id: &str) -> bool {
    system_tool("pnputil.exe").args([flag, id]).status().is_ok_and(|s| s.success())
}

fn cycle_adapter(id: &str) -> u32 {
    if !pnputil_ok("/disable-device", id) {
        return EXIT_IN_USE;
    }
    thread::sleep(OFF_TIME);
    if pnputil_ok("/enable-device", id) { EXIT_DONE } else { EXIT_NOT_BACK }
}

pub fn run_elevated_reset() -> i32 {
    for service in SERVICES {
        stop_service(service);
    }
    thread::sleep(SETTLE);
    let code = match adapter_id() {
        Some(id) => cycle_adapter(&id),
        None => EXIT_NO_ADAPTER,
    };
    for service in SERVICES.iter().rev() {
        start_service(service);
    }
    code as i32
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn reset() -> Outcome {
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    let file = wide(&exe.display().to_string());
    let verb = wide("runas");
    let args = wide(RESET_ARG);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
        return if e.code() == HRESULT::from_win32(ERROR_CANCELLED) { Outcome::Cancelled } else { Outcome::Failed(e.to_string()) };
    }
    let process: HANDLE = info.hProcess;
    let mut code = 1u32;
    unsafe {
        WaitForSingleObject(process, INFINITE);
        let _ = GetExitCodeProcess(process, &mut code);
        let _ = CloseHandle(process);
    }
    match code {
        EXIT_DONE => Outcome::Done,
        EXIT_NO_ADAPTER => Outcome::NoAdapter,
        EXIT_IN_USE => Outcome::InUse,
        EXIT_NOT_BACK => Outcome::NotBack,
        other => Outcome::Failed(format!("reset exited with code {other}")),
    }
}
