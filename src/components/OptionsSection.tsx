import { ArrowLeftRight, AudioWaveform, Power, Zap } from "lucide-react";
import { shortName } from "../lib";
import type { Change, DeviceView, StateView, SyncSound } from "../types";
import { IconButton, Section, Segmented, SettingCard, SideBadge, Switch } from "./ui";

const syncSounds: { value: SyncSound; label: string }[] = [
  { value: "silent", label: "Silent" },
  { value: "chime", label: "Chime" },
];

export function OptionsSection(props: {
  state: StateView;
  chosen: DeviceView[];
  change: Change;
  autostart: boolean;
  setAutostart: (enabled: boolean) => void;
}) {
  const { state, chosen } = props;
  const left = chosen.find((d) => d.side === "L");
  const right = chosen.find((d) => d.side === "R");
  const pairable = chosen.length === 2;
  return (
    <Section title="Options">
      <SettingCard
        icon={<ArrowLeftRight size={20} />}
        title="Stereo pair"
        detail={pairable ? "One speaker plays left, the other right" : "Turn on exactly 2 speakers to use this"}
      >
        <Switch
          checked={state.stereo}
          disabled={!pairable}
          label="Stereo pair"
          onChange={(v) => props.change("set_stereo", { enabled: v })}
        />
      </SettingCard>
      {state.stereo && left && right && (
        <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-3 rounded-lg border border-line bg-card px-4 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <SideBadge side="L" />
            <span className="truncate">{shortName(left.name)}</span>
          </div>
          <IconButton label="Swap left and right" onClick={() => props.change("swap_sides")}>
            <ArrowLeftRight size={18} />
          </IconButton>
          <div className="flex min-w-0 items-center justify-end gap-2">
            <span className="truncate">{shortName(right.name)}</span>
            <SideBadge side="R" />
          </div>
        </div>
      )}
      <SettingCard icon={<Zap size={20} />} title="Auto-sync" detail="Sync a few seconds after speakers connect">
        <Switch checked={state.autoSync} label="Auto-sync" onChange={(v) => props.change("set_auto_sync", { enabled: v })} />
      </SettingCard>
      <SettingCard
        icon={<AudioWaveform size={20} />}
        title="Sync sound"
        detail={state.syncSound === "silent" ? "A pitch too high for most people to hear" : "A short, audible chime"}
      >
        <Segmented
          value={state.syncSound}
          options={syncSounds}
          label="Sync sound"
          onChange={(sound) => props.change("set_sync_sound", { sound })}
        />
      </SettingCard>
      <SettingCard icon={<Power size={20} />} title="Start with Windows" detail="Opens in the tray when you sign in">
        <Switch checked={props.autostart} label="Start with Windows" onChange={props.setAutostart} />
      </SettingCard>
    </Section>
  );
}
