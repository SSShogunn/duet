import type { ReactNode } from "react";
import { Bug, FolderOpen, LogOut, Monitor, Moon, RotateCcw, Sun, Volume2, X } from "lucide-react";
import { applyTheme, cx } from "../lib";
import type { AppInfo, Call, Change, Theme } from "../types";
import { IconButton, Switch } from "./ui";

const themes: { value: Theme; label: string; icon: ReactNode }[] = [
  { value: "system", label: "System", icon: <Monitor size={16} /> },
  { value: "light", label: "Light", icon: <Sun size={16} /> },
  { value: "dark", label: "Dark", icon: <Moon size={16} /> },
];

function MenuItem(props: { icon: ReactNode; label: string; danger?: boolean; onClick: () => void }) {
  return (
    <button
      onClick={props.onClick}
      className={cx(
        "-mx-2 flex min-h-11 items-center gap-3 rounded-md px-2 text-left hover:bg-soft",
        props.danger && "text-error",
      )}
    >
      <span className={props.danger ? "text-error" : "text-muted"}>{props.icon}</span>
      {props.label}
    </button>
  );
}

function Label(props: { children: ReactNode }) {
  return <p className="mt-4 mb-2 text-caption font-semibold text-muted">{props.children}</p>;
}

export function SettingsMenu(props: {
  info: AppInfo | null;
  setInfo: (info: AppInfo) => void;
  call: Call;
  change: Change;
  onClose: () => void;
}) {
  const { info, setInfo, call } = props;

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
    <div className="fixed inset-0 z-10 flex justify-end bg-overlay" onClick={props.onClose}>
      <aside
        className="flex h-full w-80 animate-slide-in flex-col overflow-y-auto border-l border-line bg-bg px-5 py-4"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-title font-semibold">Settings</h2>
            <p className="text-caption text-muted">Duet {info?.version ?? ""}</p>
          </div>
          <IconButton label="Close" onClick={props.onClose}>
            <X size={20} />
          </IconButton>
        </div>

        <Label>Theme</Label>
        <div className="grid grid-cols-3 gap-1 rounded-lg border border-line bg-card p-1" role="radiogroup" aria-label="Theme">
          {themes.map((t) => {
            const selected = info?.theme === t.value;
            return (
              <button
                key={t.value}
                role="radio"
                aria-checked={selected}
                onClick={() => pickTheme(t.value)}
                className={cx(
                  "flex h-9 items-center justify-center gap-2 rounded-md",
                  selected ? "bg-accent text-on-accent" : "text-muted hover:bg-soft",
                )}
              >
                {t.icon}
                {t.label}
              </button>
            );
          })}
        </div>

        <Label>Troubleshooting</Label>
        <div className="-mx-2 flex min-h-11 items-center gap-3 px-2">
          <span className="text-muted">
            <Bug size={18} />
          </span>
          <div className="flex-1">
            <div>Debug logging</div>
            <div className="text-caption text-muted">More detail in the log file</div>
          </div>
          <Switch checked={info?.debugLogging ?? false} label="Debug logging" onChange={setDebug} />
        </div>
        <MenuItem icon={<FolderOpen size={18} />} label="Open log folder" onClick={() => call("open_log_folder")} />
        <MenuItem icon={<Volume2 size={18} />} label="Windows sound settings" onClick={() => call("open_sound_settings")} />
        <MenuItem
          icon={<RotateCcw size={18} />}
          label="Reset speaker delays"
          onClick={async () => {
            await props.change("reset_offsets");
            props.onClose();
          }}
        />

        <div className="mt-auto flex flex-col pt-4">
          <MenuItem icon={<LogOut size={18} />} label="Quit Duet" danger onClick={() => call("quit_app")} />
          {info?.logDir && <p className="mt-2 text-[12px] break-all text-muted">Logs: {info.logDir}</p>}
        </div>
      </aside>
    </div>
  );
}
