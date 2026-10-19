import { Settings } from "lucide-react";
import { useMeters } from "../meters";
import { cx, virtualLabel } from "../lib";
import type { DeviceView, StateView, Tone } from "../types";
import { AppLogo } from "./AppLogo";
import { IconButton } from "./ui";

const BARS = 28;
const BLOCK = 132;

const headlines: Record<Tone, string> = {
  ok: "In sync",
  busy: "Syncing",
  warn: "Stopped",
  idle: "Idle",
};

function subline(state: StateView, tone: Tone, chosen: DeviceView[]) {
  if (tone === "busy") return "Measuring both speakers, keep it quiet";
  if (tone === "ok") return chosen.length > 1 ? "Playing on both speakers" : "Playing on one speaker";
  if (!state.virtualSelected && state.virtualName) return `Switch Windows output to ${virtualLabel(state.virtualName)} to start`;
  if (!state.virtualName) return "Install VB-Audio Virtual Cable to start";
  if (chosen.length === 0) return "Turn on your speakers below";
  return state.status;
}

function blockOffset(tone: Tone) {
  if (tone === "ok") return 64;
  if (tone === "busy") return 40;
  return 118;
}

export function Sleeve(props: { state: StateView; tone: Tone; chosen: DeviceView[]; onSettings: () => void }) {
  const { state, tone, chosen } = props;
  const { spectrum, levels } = useMeters();
  const left = chosen[0];
  const right = chosen[1];
  const offset = blockOffset(tone);
  const live = tone === "ok" || tone === "busy";
  const scale = (d: DeviceView | undefined) => (live && d ? 0.94 + Math.min(1, (levels[d.id] ?? 0) * 1.3) * 0.06 : 0.94);
  const matched =
    chosen.length === 2 ? Math.round(Math.abs(chosen[0].offsetMs - chosen[1].offsetMs)) : null;

  return (
    <section className="relative h-[232px] overflow-hidden text-ink">
      <div aria-hidden="true" className="absolute top-[34px] right-0 isolate h-[160px] w-[260px]">
        <span
          className={cx(
            "absolute top-0 block bg-block-l mix-blend-multiply transition-[left,transform,filter,opacity] duration-700 ease-out-expo",
            !live && "opacity-70 saturate-50",
          )}
          style={{ width: BLOCK, height: BLOCK, left: 0, transform: `scale(${scale(left)})` }}
        />
        <span
          className={cx(
            "absolute top-[24px] block bg-block-r mix-blend-multiply transition-[left,transform,filter,opacity] duration-700 ease-out-expo",
            !live && "opacity-70 saturate-50",
            tone === "busy" && "animate-pulse",
          )}
          style={{ width: BLOCK, height: BLOCK, left: offset, transform: `scale(${scale(right)})` }}
        />
      </div>

      <div aria-hidden="true" className="absolute inset-x-0 bottom-0 flex h-9 items-end gap-[3px]">
        {Array.from({ length: BARS }, (_, i) => (
          <span
            key={i}
            className="flex-1 bg-ink transition-[height] duration-75 ease-out"
            style={{ height: `${Math.max(2, (spectrum[i] ?? 0) * 100)}%`, opacity: 0.16 }}
          />
        ))}
      </div>

      <header className="relative -mr-2 flex items-center gap-2 pt-1">
        <AppLogo size={22} />
        <h1 className="flex-1 text-[17px] font-black tracking-[0.12em]">DUET</h1>
        <IconButton label="Settings" onClick={props.onSettings}>
          <Settings size={19} />
        </IconButton>
      </header>

      <div className="absolute inset-x-0 bottom-12 flex items-end justify-between gap-4">
        <div className="min-w-0">
          <p className="text-headline font-black tracking-[-0.02em]">{headlines[tone]}</p>
          <p className="mt-1.5 line-clamp-2 text-caption text-muted">{subline(state, tone, chosen)}</p>
        </div>
        {matched !== null && tone === "ok" && (
          <p className="shrink-0 text-right text-caption text-muted">
            matched
            <span className="block text-[26px] leading-7 font-black text-ink">{matched} ms</span>
          </p>
        )}
      </div>
    </section>
  );
}

export function LevelBar(props: { id: string }) {
  const { levels } = useMeters();
  const level = Math.min(1, (levels[props.id] ?? 0) * 1.4);
  return (
    <span className="inline-block h-1 w-14 overflow-hidden rounded-full bg-rule align-middle" aria-hidden="true">
      <span className="block h-full rounded-full bg-ink transition-[width] duration-75 ease-out" style={{ width: `${level * 100}%` }} />
    </span>
  );
}
