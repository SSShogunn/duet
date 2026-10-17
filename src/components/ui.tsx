import type { ReactNode } from "react";
import { cx } from "../lib";

export function Switch(props: { checked: boolean; disabled?: boolean; label?: string; onChange: (value: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={props.checked}
      aria-label={props.label}
      disabled={props.disabled}
      onClick={() => props.onChange(!props.checked)}
      className={cx(
        "relative h-5 w-10 shrink-0 rounded-full border transition-colors disabled:opacity-40",
        props.checked ? "border-accent bg-accent" : "border-muted bg-transparent hover:bg-soft",
      )}
    >
      <span
        className={cx(
          "absolute top-1/2 -translate-y-1/2 rounded-full transition-all",
          props.checked ? "left-[22px] size-3.5 bg-on-accent" : "left-[3px] size-3 bg-muted",
        )}
      />
    </button>
  );
}

export function IconButton(props: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button
      aria-label={props.label}
      title={props.label}
      onClick={props.onClick}
      className="grid size-9 shrink-0 place-items-center rounded-md text-fg hover:bg-soft"
    >
      {props.children}
    </button>
  );
}

export function Button(props: { onClick: () => void; disabled?: boolean; primary?: boolean; children: ReactNode }) {
  return (
    <button
      onClick={props.onClick}
      disabled={props.disabled}
      className={cx(
        "flex h-9 shrink-0 items-center gap-2 rounded-md px-4 font-semibold disabled:opacity-40",
        props.primary ? "bg-accent text-on-accent hover:opacity-90" : "border border-line bg-card hover:bg-soft",
      )}
    >
      {props.children}
    </button>
  );
}

export function SettingCard(props: {
  icon: ReactNode;
  title: ReactNode;
  detail?: ReactNode;
  dim?: boolean;
  className?: string;
  children?: ReactNode;
}) {
  return (
    <div className={cx("flex min-h-16 items-center gap-4 rounded-lg border border-line bg-card px-4 py-3", props.className)}>
      <span className={cx("shrink-0", props.dim ? "text-muted" : "text-fg")}>{props.icon}</span>
      <div className="min-w-0 flex-1">
        <div className={cx("truncate", props.dim && "text-muted")}>{props.title}</div>
        {props.detail && <div className="text-caption text-muted">{props.detail}</div>}
      </div>
      {props.children}
    </div>
  );
}

export function Section(props: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-1">
      <h2 className="mb-1.5 px-1 text-section font-semibold">{props.title}</h2>
      {props.children}
    </section>
  );
}

export function SideBadge(props: { side: "L" | "R" }) {
  return (
    <span
      className={cx(
        "grid size-6 shrink-0 place-items-center rounded-md text-caption font-bold",
        props.side === "L" ? "bg-side-l-soft text-side-l" : "bg-side-r-soft text-side-r",
      )}
    >
      {props.side}
    </span>
  );
}
