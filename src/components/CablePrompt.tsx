import { virtualLabel } from "../lib";
import type { Call, Change, StateView } from "../types";
import { SmallButton } from "./ui";

function currentName(state: StateView) {
  return state.virtualName ? virtualLabel(state.virtualName) : "CABLE Input";
}

export function RenameSteps(props: { state: StateView; call: Call }) {
  const current = currentName(props.state);
  const blank = current === "Duet Input";
  return (
    <div className="flex items-center gap-3">
      <p className="flex-1 text-caption text-muted">
        {blank ? "Windows shows the cable with a blank name. " : ""}
        Open Sound settings → <strong className="text-ink">{blank ? "the blank output" : current}</strong> → Rename → type{" "}
        <strong className="text-ink">Duet Input</strong>.
      </p>
      <SmallButton onClick={() => props.call("open_sound_settings")}>Open</SmallButton>
    </div>
  );
}

export function CablePrompt(props: { state: StateView; call: Call; change: Change }) {
  if (!props.state.cablePrompt) return null;
  const blank = currentName(props.state) === "Duet Input";
  return (
    <section className="flex flex-col gap-1 border-y border-rule py-3">
      <div className="flex items-center justify-between gap-3">
        <p className="text-caption font-semibold">{blank ? "Fix the cable's name" : "Rename the cable to Duet Input"}</p>
        {!blank && (
          <button className="text-caption text-muted hover:text-ink" onClick={() => props.change("dismiss_cable_prompt")}>
            Not now
          </button>
        )}
      </div>
      <RenameSteps state={props.state} call={props.call} />
    </section>
  );
}
