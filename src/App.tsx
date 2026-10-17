import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Header } from "./components/Header";
import { OptionsSection } from "./components/OptionsSection";
import { SetupGuide } from "./components/SetupGuide";
import { SettingsMenu } from "./components/SettingsMenu";
import { SpeakersSection } from "./components/SpeakersSection";
import { StatusCard } from "./components/StatusCard";
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
  const [menuOpen, setMenuOpen] = useState(false);
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

  return (
    <main ref={fitRef} className="flex flex-col gap-5 px-5 pt-4 pb-5">
      <Header onMenu={() => setMenuOpen(true)} />
      <SetupGuide state={state} chosenCount={chosen.length} call={call} />
      <StatusCard state={state} tone={tone} canSync={canSync} error={error} call={call} />
      <SpeakersSection speakers={speakers} change={change} />
      <OptionsSection state={state} chosen={chosen} change={change} autostart={autostart} setAutostart={setAutostart} />
      <p className="text-center text-caption text-muted">Closing the window keeps Duet running in the tray.</p>
      {menuOpen && (
        <SettingsMenu info={info} setInfo={setInfo} call={call} change={change} onClose={() => setMenuOpen(false)} />
      )}
    </main>
  );
}

export default App;
