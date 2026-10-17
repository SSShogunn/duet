use serde::Serialize;
use wasapi::{DeviceEnumerator, Direction, WasapiError};
use windows::core::HSTRING;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

pub const VIRTUAL_MARKER: &str = "VB-Audio Virtual Cable";

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_virtual: bool,
}

pub fn is_virtual(name: &str) -> bool {
    name.contains(VIRTUAL_MARKER)
}

pub fn list_outputs() -> Result<Vec<OutputDevice>, WasapiError> {
    let enumerator = DeviceEnumerator::new()?;
    let default_id = enumerator
        .get_default_device(&Direction::Render)
        .and_then(|d| d.get_id())
        .unwrap_or_default();
    let collection = enumerator.get_device_collection(&Direction::Render)?;
    let mut devices = Vec::new();
    for index in 0..collection.get_nbr_devices()? {
        let device = collection.get_device_at_index(index)?;
        let id = device.get_id()?;
        let name = device.get_friendlyname()?;
        devices.push(OutputDevice {
            is_default: id == default_id,
            is_virtual: is_virtual(&name),
            name,
            id,
        });
    }
    Ok(devices)
}

pub fn default_input_id() -> Option<String> {
    let enumerator = DeviceEnumerator::new().ok()?;
    if let Ok(device) = enumerator.get_default_device(&Direction::Capture) {
        let name = device.get_friendlyname().unwrap_or_default();
        if !is_virtual(&name) {
            return device.get_id().ok();
        }
    }
    let collection = enumerator.get_device_collection(&Direction::Capture).ok()?;
    for index in 0..collection.get_nbr_devices().ok()? {
        let device = collection.get_device_at_index(index).ok()?;
        if !is_virtual(&device.get_friendlyname().unwrap_or_default()) {
            return device.get_id().ok();
        }
    }
    None
}

pub struct EndpointVolume {
    volume: IAudioEndpointVolume,
}

impl EndpointVolume {
    pub fn open(device_id: &str) -> windows::core::Result<EndpointVolume> {
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDevice(&HSTRING::from(device_id))?;
            let volume = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)?;
            Ok(EndpointVolume { volume })
        }
    }

    pub fn gain(&self) -> windows::core::Result<f32> {
        unsafe {
            if self.volume.GetMute()?.as_bool() {
                return Ok(0.0);
            }
            let db = self.volume.GetMasterVolumeLevel()?;
            Ok(10f32.powf(db / 20.0))
        }
    }
}
