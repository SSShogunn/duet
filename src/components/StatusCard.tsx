import { CircleCheck, CirclePause, RefreshCw, TriangleAlert } from "lucide-react";
import { cx, shortName, toneClass } from "../lib";
import type { Call, StateView, Tone } from "../types";
import { Button } from "./ui";

const icons = { ok: CircleCheck, busy: RefreshCw, warn: TriangleAlert, idle: CirclePause };
const headlines: Record<Tone, string> = {
  ok: "Playing",
  busy: "Syncing, stay quiet",
  warn: "Audio stopped",
  idle: "Idle",
};

export function StatusCard(props: { state: StateView; tone: Tone; canSync: boolean; error: string | null; call: Call }) {
  const { state, tone } = props;
  const Icon = icons[tone];
  return (
    <section className="flex items-center gap-4 rounded-lg border border-line bg-card px-4 py-4">
      <span className={cx("grid size-11 shrink-0 place-items-center rounded-full", toneClass[tone])}>
        <Icon size={22} className={cx(state.syncing && "animate-spin")} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="font-semibold">{headlines[tone]}</div>
        <div className="text-caption text-muted">{state.status}</div>
        {!state.virtualSelected && state.virtualName && (
          <div className="text-caption text-muted">
            Set Windows output to <strong className="text-fg">{shortName(state.virtualName)}</strong>
          </div>
        )}
        {state.lastSync && !state.syncing && <div className="text-caption text-muted">{state.lastSync}</div>}
        {props.error && <div className="text-caption text-error">{props.error}</div>}
      </div>
      <Button primary disabled={!props.canSync} onClick={() => props.call("sync_now")}>
        <RefreshCw size={16} className={cx(state.syncing && "animate-spin")} />
        Sync
      </Button>
    </section>
  );
}
