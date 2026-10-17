import { Minus, Plus, Speaker } from "lucide-react";
import { shortName } from "../lib";
import type { Change, DeviceView } from "../types";
import { Section, SettingCard, SideBadge, Switch } from "./ui";

function DelayStepper(props: { device: DeviceView; change: Change }) {
  const { id, offsetMs } = props.device;
  const step = (ms: number) => props.change("set_offset", { id, ms });
  return (
    <div className="flex items-center rounded-md border border-line tabular-nums" title="Extra delay for this speaker">
      <button aria-label="Less delay" className="grid size-8 place-items-center rounded-l-md hover:bg-soft" onClick={() => step(offsetMs - 5)}>
        <Minus size={14} />
      </button>
      <span className="min-w-14 text-center text-caption">{offsetMs.toFixed(0)} ms</span>
      <button aria-label="More delay" className="grid size-8 place-items-center rounded-r-md hover:bg-soft" onClick={() => step(offsetMs + 5)}>
        <Plus size={14} />
      </button>
    </div>
  );
}

function detail(d: DeviceView) {
  if (!d.enabled) return "Not used";
  if (d.side === "L") return "Left channel";
  if (d.side === "R") return "Right channel";
  return "Playing";
}

export function SpeakersSection(props: { speakers: DeviceView[]; change: Change }) {
  return (
    <Section title="Speakers">
      {props.speakers.length === 0 && <SettingCard icon={<Speaker size={20} />} title="No output devices found" dim />}
      {props.speakers.map((d) => (
        <SettingCard
          key={d.id}
          icon={<Speaker size={20} />}
          dim={!d.enabled}
          title={
            <span className="flex items-center gap-2">
              <span className="truncate">{shortName(d.name)}</span>
              {d.side && <SideBadge side={d.side} />}
            </span>
          }
          detail={detail(d)}
        >
          {d.enabled && <DelayStepper device={d} change={props.change} />}
          <Switch
            checked={d.enabled}
            label={`Use ${shortName(d.name)}`}
            onChange={(v) => props.change("set_target", { id: d.id, enabled: v })}
          />
        </SettingCard>
      ))}
    </Section>
  );
}
