import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RefreshCw, Settings } from "lucide-react";
import { SettingsHeader } from "./components/Header";
import { CablePrompt } from "./components/CablePrompt";
import { SetupGuide } from "./components/SetupGuide";
import { SettingsPage } from "./components/SettingsPage";
import { Sleeve } from "./components/Sleeve";
import { SpeakersSection } from "./components/SpeakersSection";
import { ActionBar } from "./components/ui";
import { cx } from "./lib";
import { applyTheme } from "./lib";
import type { AppInfo, Args, StateView, Tone } from "./types";

function toneOf(state: StateView): Tone {
  if (state.syncing) return "busy";
  if (state.running) return "ok";
  if (state.status.startsWith("Audio stopped")) return "warn";
  return "idle";
}

function useFitWindow() {
  const fitted = useRef(false);
  const observer = useRef<ResizeObserver | null>(null);
  return useCallback((el: HTMLElement | null) => {
    observer.current?.disconnect();
    if (!el) return;
    observer.current = new ResizeObserver(() => {
      const first = !fitted.current;
      fitted.current = true;
      invoke("fit_window", { height: Math.ceil(el.getBoundingClientRect().height), first }).catch(() => {});
    });
    observer.current.observe(el);
  }, []);
}

function App() {
  const [state, setState] = useState<StateView | null>(null);
  const [autostart, setAutostartState] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<"main" | "settings">("main");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const fitRef = useFitWindow();

  async function call<T>(command: string, args?: Args): Promise<T | null> {
    try {
      const result = await invoke<T>(command, args);
      setError(null);
      return result;
    } catch (e) {
      console.error(command, e);
      setError(typeof e === "string" ? e : "Something went wrong. Details are in the log.");
      return null;
    }
  }

  async function change(command: string, args?: Args) {
    const next = await call<StateView>(command, args);
    if (next) setState(next);
  }

  async function setAutostart(enabled: boolean) {
    const result = await call<boolean>("set_autostart", { enabled });
    if (result !== null) setAutostartState(result);
  }

  useEffect(() => {
    if (!error) return;
    const timer = setTimeout(() => setError(null), 8000);
    return () => clearTimeout(timer);
  }, [error]);

  useEffect(() => {
    call<StateView>("get_state").then((s) => s && setState(s));
    call<boolean>("get_autostart").then((v) => v !== null && setAutostartState(v));
    call<AppInfo>("get_app_info").then((v) => {
      if (!v) return;
      setInfo(v);
      applyTheme(v.theme);
    });
    const unlisten = listen<StateView>("state", (event) => setState(event.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  if (!state) return null;

  const speakers = state.devices.filter((d) => !d.isVirtual);
  const chosen = speakers.filter((d) => d.enabled);
  const canSync = state.running && chosen.length > 1 && !state.syncing;
  const tone = toneOf(state);

  return view === "settings" ? (
    <main className="flex h-screen flex-col gap-3 px-5 pt-3 pb-5">
      <SettingsHeader version={info?.version} onBack={() => setView("main")} />
      <SettingsPage
        state={state}
        info={info}
        setInfo={setInfo}
        autostart={autostart}
        setAutostart={setAutostart}
        call={call}
        change={change}
      />
    </main>
  ) : (
    <main ref={fitRef} className="flex flex-col gap-4 px-4 pt-4 pb-5">
      <div className="flex flex-col gap-2">
        <Sleeve state={state} tone={tone} chosen={chosen} onSettings={() => setView("settings")} />
        {state.virtualName && !state.virtualSelected ? (
          <ActionBar onClick={() => call("open_sound_settings")}>
            <Settings size={16} />
            Open sound settings
          </ActionBar>
        ) : (
          <ActionBar disabled={!canSync} onClick={() => call("sync_now")}>
            <RefreshCw size={16} className={cx(state.syncing && "animate-spin")} />
            {state.syncing ? "Syncing" : "Sync now"}
          </ActionBar>
        )}
        {state.unstableSpeaker && (
          <p className="text-caption text-warn">{state.unstableSpeaker} keeps dropping out. Turn it off and on, or move it closer.</p>
        )}
        {error && <p className="text-caption text-error">{error}</p>}
      </div>
      <SetupGuide state={state} chosenCount={chosen.length} call={call} />
      <CablePrompt state={state} call={call} change={change} />
      <SpeakersSection state={state} speakers={speakers} chosen={chosen} change={change} />
      <p className="text-center text-caption text-muted">
        {state.lastSync ?? "Closing the window keeps Duet running in the tray."}
      </p>
    </main>
  );
}

export default App;
