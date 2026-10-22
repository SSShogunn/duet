import { useState } from "react";
import { applyTheme, cx, virtualLabel } from "../lib";
import { RenameSteps } from "./CablePrompt";
import type { AppInfo, Call, Change, StateView, SyncSound, Theme } from "../types";
import { LinkRow, Row, Segmented, Switch } from "./ui";

type Tab = "sync" | "general" | "help";

const tabs: { value: Tab; label: string; hue: string }[] = [
  { value: "sync", label: "Sync", hue: "border-block-l" },
  { value: "general", label: "General", hue: "border-block-r" },
  { value: "help", label: "Help", hue: "border-field" },
];

const syncSounds: { value: SyncSound; label: string }[] = [
  { value: "silent", label: "Silent" },
  { value: "chime", label: "Chime" },
];

const intervals: { value: string; label: string }[] = [
  { value: "0", label: "Off" },
  { value: "5", label: "5m" },
  { value: "15", label: "15m" },
  { value: "30", label: "30m" },
];

const themes: { value: Theme; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

export function SettingsPage(props: {
  state: StateView;
  info: AppInfo | null;
  setInfo: (info: AppInfo) => void;
  autostart: boolean;
  setAutostart: (enabled: boolean) => void;
  call: Call;
  change: Change;
}) {
  const { state, info, setInfo, call, change } = props;
  const [tab, setTab] = useState<Tab>("sync");
  const chime = state.syncSound === "chime";

  async function pickTheme(theme: Theme) {
    applyTheme(theme);
    const result = await call<Theme>("set_theme", { theme });
    if (result && info) setInfo({ ...info, theme: result });
  }

  async function setDebug(enabled: boolean) {
    const result = await call<boolean>("set_debug_logging", { enabled });
    if (result !== null && info) setInfo({ ...info, debugLogging: result });
  }

  return (
    <div className="flex flex-1 flex-col">
      <div className="grid grid-cols-3 border-b border-rule" role="tablist" aria-label="Settings sections">
        {tabs.map((t) => (
          <button
            key={t.value}
            role="tab"
            aria-selected={tab === t.value}
            onClick={() => setTab(t.value)}
            className={cx(
              "-mb-px h-10 border-b-[3px] transition-colors",
              tab === t.value ? cx("font-bold", t.hue) : "border-transparent text-muted hover:text-ink",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "sync" && (
        <div>
          <Row title="Auto-sync" detail="Sync a few seconds after speakers connect">
            <Switch checked={state.autoSync} label="Auto-sync" onChange={(v) => change("set_auto_sync", { enabled: v })} />
          </Row>
          <Row title="Sync sound" detail={chime ? "A short chime you can hear" : "Too high-pitched for most people to hear"}>
            <Segmented value={state.syncSound} options={syncSounds} label="Sync sound" onChange={(sound) => change("set_sync_sound", { sound })} />
          </Row>
          <Row title="Background sync" detail={chime ? "Needs the silent sync sound" : "Re-syncs quietly while you listen"}>
            <Segmented
              value={String(state.backgroundSyncMin)}
              options={intervals}
              label="Background sync"
              disabled={chime}
              onChange={(minutes) => change("set_background_sync", { minutes: Number(minutes) })}
            />
          </Row>
        </div>
      )}

      {tab === "general" && (
        <div>
          <Row title="Theme">
            <Segmented value={info?.theme ?? "system"} options={themes} label="Theme" onChange={pickTheme} />
          </Row>
          <Row title="Start with Windows" detail="Opens in the tray when you sign in">
            <Switch checked={props.autostart} label="Start with Windows" onChange={props.setAutostart} />
          </Row>
          {state.virtualName && state.cableRenamed && <Row title="Cable name" detail={virtualLabel(state.virtualName)} />}
          {state.virtualName && !state.cableRenamed && (
            <div className="border-b border-rule py-2.5">
              <p className="font-semibold">Cable name</p>
              <RenameSteps state={state} call={call} />
            </div>
          )}
          <LinkRow title="Windows sound settings" detail="Pick outputs and volumes" onClick={() => call("open_sound_settings")} />
        </div>
      )}

      {tab === "help" && (
        <div>
          <Row title="Debug logging" detail="More detail in the log file">
            <Switch checked={info?.debugLogging ?? false} label="Debug logging" onChange={setDebug} />
          </Row>
          <LinkRow
            title="Reset Bluetooth"
            detail={state.bluetoothResetting ? "Resetting, this takes about 15 seconds" : "Clears crackling without a restart. Sound cuts out briefly"}
            onClick={() => !state.bluetoothResetting && call("reset_bluetooth")}
          />
          <LinkRow title="Open log folder" onClick={() => call("open_log_folder")} />
          <LinkRow title="Reset speaker delays" detail="Then press Sync to measure again" onClick={() => change("reset_offsets")} />
        </div>
      )}

      <div className="mt-auto pt-6">
        <LinkRow title="Quit Duet" detail="Stops playing on your speakers" danger onClick={() => call("quit_app")} />
      </div>
    </div>
  );
}
