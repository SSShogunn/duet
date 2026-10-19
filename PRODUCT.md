# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Windows PC owners with two speakers, usually two Bluetooth speakers such as Echo Dots, who want one sound filling the room instead of a single speaker. Duet is a public release, so the first-time user has never heard of virtual audio cables and needs guidance through setup.

## Product Purpose

Duet plays the PC's audio on two speakers at once and keeps them in sync so they sound like one system, not an echo. Success means the user sets it up once, forgets it, and the two speakers stay aligned for music and video.

## Positioning

Windows plays sound on one output at a time, and Bluetooth speakers each add their own shifting delay. Duet measures the delay between the speakers with the laptop microphone, using a near-ultrasonic signal most adults cannot hear, plays it over the music without pausing it, and keeps correcting drift in the background.

## Operating Context

- Mostly music and movies or video. Lag between speakers is noticeable with video, so sync quality matters.
- Set and forget: opened to set up, then it lives in the Windows tray. The window is mainly a glanceable status check: playing, in sync, which speakers.
- Requires the VB-Audio Virtual Cable driver, selected as the Windows sound output. The app guides this setup.
- Desktop app built with Tauri 2: a fixed-width window (about 480 px) whose height fits the main view, with no resizing or scrolling. The settings view opens inside the same window size.

## Capabilities and Constraints

- Up to 2 speakers, with an optional stereo pair (left and right) and a swap.
- Sync: Silent (19–20.8 kHz) or Chime, auto-sync on connect, and background sync every 5, 15 or 30 minutes.
- Per-speaker manual delay, live per-speaker level and a live spectrum of the playing audio.
- Light, dark and system themes. Start with Windows. Debug logging and a log folder.
- Errors are shown as plain messages. Raw Windows errors go to the log only.
- Windows only.

## Brand Commitments

- Name: Duet.
- Logo: a centre dot with sound waves going out both ways (`app-icon.svg`).
- Voice: plain, short, friendly, no jargon.

## Evidence on Hand

No screenshots, testimonials or user counts exist yet. Do not fabricate them.

## Product Principles

1. Glance, don't study: the main view answers "is it playing and in sync?" instantly.
2. Never interrupt the music: sync and corrections stay invisible unless something needs the user.
3. Guide first-timers without slowing returning users.
4. Fit the window: everything belongs in one fixed-size view, with no scrolling.

## Accessibility & Inclusion

Not specified beyond normal readable text, keyboard focus and contrast in both themes.
