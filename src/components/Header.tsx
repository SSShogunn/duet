import { Menu } from "lucide-react";
import { AppLogo } from "./AppLogo";
import { IconButton } from "./ui";

export function Header(props: { onMenu: () => void }) {
  return (
    <header className="flex items-center gap-3">
      <span className="grid size-10 shrink-0 place-items-center rounded-lg bg-accent text-on-accent">
        <AppLogo size={30} />
      </span>
      <div className="flex-1">
        <h1 className="text-title font-semibold">Audio Split</h1>
        <p className="text-caption text-muted">Play your PC audio on every speaker</p>
      </div>
      <IconButton label="Menu" onClick={props.onMenu}>
        <Menu size={20} />
      </IconButton>
    </header>
  );
}
