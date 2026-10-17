use std::fmt::Display;

const KNOWN: &[(&str, &str)] = &[
    ("0x88890004", "A speaker was disconnected. Audio will resume when it's back."),
    ("0x80070490", "A speaker is no longer available. Audio will resume when it's back."),
    ("0x8889000A", "A speaker is being used by another app."),
    ("0x88890010", "The Windows Audio service isn't running."),
    ("0x88890008", "A speaker doesn't support this audio format."),
    ("0x80070005", "Windows blocked access to an audio device."),
    ("stopped responding", "A speaker stopped responding. Reconnecting..."),
];

pub fn friendly(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    KNOWN
        .iter()
        .find(|(code, _)| lower.contains(&code.to_ascii_lowercase()))
        .map(|(_, message)| message.to_string())
        .unwrap_or_else(|| "Something went wrong with the audio. Retrying...".to_string())
}

pub fn report(context: &str, error: impl Display, message: &str) -> String {
    log::error!("{context}: {error}");
    message.to_string()
}
