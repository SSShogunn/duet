use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const CONSENT_STORE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

fn recording_now(key: &RegKey) -> bool {
    let start = key.get_value::<u64, _>("LastUsedTimeStart").unwrap_or(0);
    let stop = key.get_value::<u64, _>("LastUsedTimeStop").unwrap_or(1);
    start > 0 && stop == 0
}

pub fn other_app_using_microphone() -> Option<String> {
    let own = std::env::current_exe().ok().map(|p| p.to_string_lossy().replace('\\', "#").to_lowercase());
    let root = RegKey::predef(HKEY_CURRENT_USER).open_subkey(CONSENT_STORE).ok()?;
    for name in root.enum_keys().flatten() {
        let Ok(key) = root.open_subkey(&name) else { continue };
        if name == "NonPackaged" {
            for app in key.enum_keys().flatten() {
                if own.as_deref() == Some(app.to_lowercase().as_str()) {
                    continue;
                }
                if key.open_subkey(&app).is_ok_and(|k| recording_now(&k)) {
                    return Some(app);
                }
            }
        } else if recording_now(&key) {
            return Some(name);
        }
    }
    None
}

#[cfg(test)]
mod hardware {
    #[test]
    #[ignore]
    fn reports_microphone_users() {
        println!("microphone in use by: {:?}", super::other_app_using_microphone());
    }
}
