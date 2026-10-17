import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type DeviceView = {
  id: string;
  name: string;
  isDefault: boolean;
  isVirtual: boolean;
  enabled: boolean;
  offsetMs: number;
  side: string | null;
};

type StateView = {
  status: string;
  running: boolean;
  syncing: boolean;
  virtualSelected: boolean;
  virtualName: string | null;
  devices: DeviceView[];
  stereo: boolean;
  swap: boolean;
  autoSync: boolean;
  lastSync: string | null;
  glitches: number;
};

function shortName(name: string) {
  return name.replace(/^Speakers \(/, "").replace(/\)$/, "");
}

function App() {
  const [state, setState] = useState<StateView | null>(null);
  const [autostart, setAutostart] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function call<T>(command: string, args?: Record<string, unknown>) {
    try {
      const result = await invoke<T>(command, args);
      setError(null);
      return result;
    } catch (e) {
      setError(String(e));
      return null;
    }
  }

  async function change(command: string, args?: Record<string, unknown>) {
    const next = await call<StateView>(command, args);
    if (next) setState(next);
  }

  useEffect(() => {
    call<StateView>("get_state").then((s) => s && setState(s));
    call<boolean>("get_autostart").then((v) => v !== null && setAutostart(v));
    const unlisten = listen<StateView>("state", (event) => setState(event.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  if (!state) return <main className="app">Loading…</main>;

  const speakers = state.devices.filter((d) => !d.isVirtual);
  const chosen = speakers.filter((d) => d.enabled);
  const canSync = state.running && chosen.length > 1 && !state.syncing;

  return (
    <main className="app">
      <header>
        <h1>Audio Split</h1>
        <span className={`dot ${state.running ? "on" : ""}`} />
      </header>

      <section className="card status">
        <p>{state.status}</p>
        {!state.virtualSelected && state.virtualName && (
          <p className="hint">
            Windows sound output → <strong>{state.virtualName}</strong>
          </p>
        )}
        {error && <p className="error">{error}</p>}
      </section>

      <section className="card">
        <h2>Speakers</h2>
        {speakers.length === 0 && <p className="hint">No output devices found.</p>}
        {speakers.map((d) => (
          <div key={d.id} className="speaker">
            <label>
              <input
                type="checkbox"
                checked={d.enabled}
                onChange={(e) => change("set_target", { id: d.id, enabled: e.target.checked })}
              />
              <span>{shortName(d.name)}</span>
              {d.side && <span className="badge">{d.side}</span>}
            </label>
            {d.enabled && (
              <div className="delay">
                <button onClick={() => change("set_offset", { id: d.id, ms: d.offsetMs - 5 })}>−</button>
                <span>{d.offsetMs.toFixed(0)} ms</span>
                <button onClick={() => change("set_offset", { id: d.id, ms: d.offsetMs + 5 })}>+</button>
              </div>
            )}
          </div>
        ))}
      </section>

      <section className="card">
        <h2>Sync</h2>
        <button className="primary" disabled={!canSync} onClick={() => call("sync_now")}>
          {state.syncing ? "Syncing… stay quiet" : "Sync speakers now"}
        </button>
        {state.lastSync && <p className="hint">{state.lastSync}</p>}
        <label className="toggle">
          <input type="checkbox" checked={state.autoSync} onChange={(e) => change("set_auto_sync", { enabled: e.target.checked })} />
          Auto-sync when speakers connect
        </label>
      </section>

      <section className="card">
        <h2>Options</h2>
        <label className="toggle">
          <input
            type="checkbox"
            checked={state.stereo}
            disabled={chosen.length !== 2}
            onChange={(e) => change("set_stereo", { enabled: e.target.checked })}
          />
          Stereo pair (left / right){chosen.length !== 2 ? " — needs exactly 2 speakers" : ""}
        </label>
        {state.stereo && chosen.length === 2 && (
          <button onClick={() => change("swap_sides")}>Swap left and right</button>
        )}
        <label className="toggle">
          <input
            type="checkbox"
            checked={autostart}
            onChange={async (e) => {
              const v = await call<boolean>("set_autostart", { enabled: e.target.checked });
              if (v !== null) setAutostart(v);
            }}
          />
          Start with Windows
        </label>
      </section>

      <footer>
        Closing this window keeps Audio Split running in the tray.
        {state.glitches > 0 && ` · ${state.glitches} audio glitches this session`}
      </footer>
    </main>
  );
}

export default App;
