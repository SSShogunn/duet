use serde::Serialize;
use wasapi::{DeviceEnumerator, Direction, WasapiError};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

fn read_outputs() -> Result<Vec<OutputDevice>, WasapiError> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new()?;
    let default_id = enumerator.get_default_device(&Direction::Render)?.get_id()?;
    let collection = enumerator.get_device_collection(&Direction::Render)?;

    let mut devices = Vec::new();
    for index in 0..collection.get_nbr_devices()? {
        let device = collection.get_device_at_index(index)?;
        let id = device.get_id()?;
        devices.push(OutputDevice {
            is_default: id == default_id,
            name: device.get_friendlyname()?,
            id,
        });
    }
    Ok(devices)
}

#[tauri::command]
pub fn list_outputs() -> Result<Vec<OutputDevice>, String> {
    std::thread::spawn(read_outputs)
        .join()
        .map_err(|_| "audio thread crashed".to_string())?
        .map_err(|e| e.to_string())
}
