import type { ReactNode } from "react";
import { Check, Download, Settings } from "lucide-react";
import { cx, virtualLabel } from "../lib";
import type { Call, StateView } from "../types";
import { Button } from "./ui";

type Step = {
  title: ReactNode;
  detail: string;
  done: boolean;
  action?: ReactNode;
};

function StepRow(props: { step: Step; number: number; current: boolean }) {
  const { step } = props;
  return (
    <li className="flex items-start gap-3">
      <span
        className={cx(
          "mt-0.5 grid size-6 shrink-0 place-items-center rounded-full text-caption font-semibold",
          step.done ? "bg-ok-soft text-ok" : props.current ? "bg-accent text-on-accent" : "bg-soft text-muted",
        )}
      >
        {step.done ? <Check size={14} /> : props.number}
      </span>
      <div className="min-w-0 flex-1">
        <div className={cx(step.done ? "text-muted line-through" : !props.current && "text-muted")}>{step.title}</div>
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
        <Button primary onClick={() => props.call("open_vb_cable_page")}>
          <Download size={16} />
          Get it
        </Button>
      ),
    },
    {
      title: (
        <>
          Set Windows sound output to <strong>{output}</strong>
        </>
      ),
      detail: "Pick it under Output in Windows sound settings.",
      done: state.virtualSelected,
      action: (
        <Button primary onClick={() => props.call("open_sound_settings")}>
          <Settings size={16} />
          Open
        </Button>
      ),
    },
    {
      title: "Turn on your speakers below",
      detail: "Switch on each speaker Duet should play on.",
      done: props.chosenCount > 0,
    },
  ];
  const current = steps.findIndex((s) => !s.done);
  if (current === -1) return null;

  return (
    <section className="flex flex-col gap-3 rounded-lg border border-line bg-card px-4 py-4">
      <div>
        <h2 className="font-semibold">Set up Duet</h2>
        <p className="text-caption text-muted">
          Step {current + 1} of {steps.length}
        </p>
      </div>
      <ol className="flex flex-col gap-3">
        {steps.map((step, i) => (
          <StepRow key={i} step={step} number={i + 1} current={i === current} />
        ))}
      </ol>
    </section>
  );
}
