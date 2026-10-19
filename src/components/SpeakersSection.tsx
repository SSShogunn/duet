import { ArrowLeftRight, Minus, Plus } from "lucide-react";
import { shortName } from "../lib";
import type { Change, DeviceView, StateView } from "../types";
import { LevelBar } from "./Sleeve";
import { Dot, IconButton, List, Row, Switch } from "./ui";

function DelayStepper(props: { device: DeviceView; change: Change }) {
  const { id, offsetMs } = props.device;
  const step = (ms: number) => props.change("set_offset", { id, ms });
  return (
    <div className="flex items-center" title="Extra delay for this speaker">
      <button aria-label="Less delay" className="grid size-7 place-items-center rounded-[4px] hover:bg-soft" onClick={() => step(offsetMs - 5)}>
        <Minus size={14} />
      </button>
      <span className="min-w-12 text-center text-caption font-semibold">{offsetMs.toFixed(0)} ms</span>
      <button aria-label="More delay" className="grid size-7 place-items-center rounded-[4px] hover:bg-soft" onClick={() => step(offsetMs + 5)}>
        <Plus size={14} />
      </button>
    </div>
  );
}

function sideLabel(d: DeviceView) {
  if (d.side === "L") return "Left";
  if (d.side === "R") return "Right";
  return "Both channels";
}

function StereoControl(props: { state: StateView; pairable: boolean; change: Change }) {
  return (
    <div className="flex items-center gap-2" title={props.pairable ? undefined : "Turn on two speakers to pair them"}>
      {props.state.stereo && props.pairable && (
        <IconButton label="Swap left and right" onClick={() => props.change("swap_sides")}>
          <ArrowLeftRight size={16} />
        </IconButton>
      )}
      <span className={props.pairable ? "text-caption font-semibold" : "text-caption text-muted"}>Stereo pair</span>
      <Switch
        checked={props.state.stereo}
        disabled={!props.pairable}
        label="Stereo pair"
        onChange={(v) => props.change("set_stereo", { enabled: v })}
      />
    </div>
  );
}

export function SpeakersSection(props: { state: StateView; speakers: DeviceView[]; chosen: DeviceView[]; change: Change }) {
  const full = props.chosen.length >= 2;
  return (
    <List title="Speakers" action={<StereoControl state={props.state} pairable={props.chosen.length === 2} change={props.change} />}>
      {props.speakers.length === 0 && <Row title="No speakers found" detail="Connect a speaker in Windows Bluetooth settings" dim />}
      {props.speakers.map((d) => {
        const index = props.chosen.findIndex((c) => c.id === d.id);
        const color = index === 0 ? "l" : index === 1 ? "r" : "none";
        return (
          <Row
            key={d.id}
            lead={<Dot color={color} />}
            dim={!d.enabled}
            title={shortName(d.name)}
            detail={
              d.enabled ? (
                <span className="flex items-center gap-2">
                  {sideLabel(d)}
                  <LevelBar id={d.id} />
                </span>
              ) : full ? (
                "Turn one speaker off to use this one"
              ) : (
                "Off"
              )
            }
          >
            {d.enabled && <DelayStepper device={d} change={props.change} />}
            <Switch
              checked={d.enabled}
              disabled={!d.enabled && full}
              label={`Use ${shortName(d.name)}`}
              onChange={(v) => props.change("set_target", { id: d.id, enabled: v })}
            />
          </Row>
        );
      })}
    </List>
  );
}
