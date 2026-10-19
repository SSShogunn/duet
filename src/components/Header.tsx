import { ArrowLeft } from "lucide-react";
import { IconButton } from "./ui";

export function SettingsHeader(props: { version: string | undefined; onBack: () => void }) {
  return (
    <header className="flex items-center gap-2">
      <IconButton label="Back" onClick={props.onBack}>
        <ArrowLeft size={20} />
      </IconButton>
      <h1 className="flex-1 text-[22px] font-black tracking-[-0.01em]">Settings</h1>
      <span className="text-caption text-muted">Duet {props.version ?? ""}</span>
    </header>
  );
}
