import type { ReactNode } from "react";
import { Check, Download, Settings } from "lucide-react";
import { cx, virtualLabel } from "../lib";
import type { Call, StateView } from "../types";
import { SmallButton } from "./ui";

type Step = {
  title: ReactNode;
  detail: string;
  done: boolean;
  action?: ReactNode;
};

function StepRow(props: { step: Step; number: number; current: boolean }) {
  const { step } = props;
  return (
    <li className="flex min-h-14 items-center gap-3 border-b border-rule py-2.5">
      <span
        className={cx(
          "grid size-6 shrink-0 place-items-center rounded-full text-caption font-bold",
          step.done ? "bg-ok text-bg" : props.current ? "bg-ink text-bg" : "border-[1.5px] border-muted text-muted",
        )}
      >
        {step.done ? <Check size={14} strokeWidth={3} /> : props.number}
      </span>
      <div className="min-w-0 flex-1">
        <div className={cx(props.current ? "font-semibold" : "text-muted", step.done && "line-through")}>{step.title}</div>
        {props.current && <div className="text-caption text-muted">{step.detail}</div>}
      </div>
      {props.current && step.action}
    </li>
  );
}

export function SetupGuide(props: { state: StateView; chosenCount: number; call: Call }) {
  const { state } = props;
  const output = state.virtualName ? virtualLabel(state.virtualName) : "the VB-Cable output";
  const steps: Step[] = [
    {
      title: "Install VB-Audio Virtual Cable",
      detail: "A free driver Duet listens to. Install it, then restart your PC.",
      done: state.virtualName !== null,
      action: (
        <SmallButton onClick={() => props.call("open_vb_cable_page")}>
          <Download size={14} />
          Get it
        </SmallButton>
      ),
    },
    {
      title: (
        <>
          Set Windows output to <strong>{output}</strong>
        </>
      ),
      detail: "Pick it under Output in Windows sound settings.",
      done: state.virtualSelected,
      action: (
        <SmallButton onClick={() => props.call("open_sound_settings")}>
          <Settings size={14} />
          Open
        </SmallButton>
      ),
    },
    {
      title: "Turn on two speakers below",
      detail: "Switch on the speakers Duet should play on.",
      done: props.chosenCount > 0,
    },
  ];
  const current = steps.findIndex((s) => !s.done);
  if (state.virtualName !== null || current === -1) return null;

  return (
    <section>
      <div className="flex min-h-9 items-center justify-between pb-1">
        <h2 className="font-bold">Set up Duet</h2>
        <span className="text-caption text-muted">
          Step {current + 1} of {steps.length}
        </span>
      </div>
      <ol className="border-t-2 border-ink">
        {steps.map((step, i) => (
          <StepRow key={i} step={step} number={i + 1} current={i === current} />
        ))}
      </ol>
    </section>
  );
}
