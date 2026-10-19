import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";
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
        "relative h-5 w-[38px] shrink-0 rounded-full border-[1.5px] transition-colors duration-200 disabled:opacity-35",
        props.checked ? "border-ink bg-ink" : "border-muted bg-transparent hover:bg-soft",
      )}
    >
      <span
        className={cx(
          "absolute top-1/2 size-3 -translate-y-1/2 rounded-full transition-[left,background-color] duration-200 ease-out-expo",
          props.checked ? "left-[20px] bg-bg" : "left-[3px] bg-muted",
        )}
      />
    </button>
  );
}

export function IconButton(props: { label: string; onClick: () => void; onField?: boolean; children: ReactNode }) {
  return (
    <button
      aria-label={props.label}
      title={props.label}
      onClick={props.onClick}
      className={cx(
        "grid size-9 shrink-0 place-items-center rounded-md transition-colors",
        props.onField ? "text-on-field hover:bg-on-field/15" : "text-ink hover:bg-soft",
      )}
    >
      {props.children}
    </button>
  );
}

export function ActionBar(props: { onClick: () => void; disabled?: boolean; children: ReactNode }) {
  return (
    <button
      onClick={props.onClick}
      disabled={props.disabled}
      className="flex h-11 w-full items-center justify-center gap-2 rounded-[4px] bg-action font-semibold text-on-action transition-opacity hover:opacity-90 disabled:opacity-40"
    >
      {props.children}
    </button>
  );
}

export function SmallButton(props: { onClick: () => void; children: ReactNode }) {
  return (
    <button
      onClick={props.onClick}
      className="flex h-8 shrink-0 items-center gap-1.5 rounded-[4px] border border-ink px-3 text-caption font-semibold hover:bg-soft"
    >
      {props.children}
    </button>
  );
}

export function List(props: { title?: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section>
      {(props.title || props.action) && (
        <div className="flex min-h-9 items-center justify-between pb-1">
          {props.title && <h2 className="font-bold">{props.title}</h2>}
          {props.action}
        </div>
      )}
      <div className="border-t-2 border-ink">{props.children}</div>
    </section>
  );
}

export function Row(props: { lead?: ReactNode; title: ReactNode; detail?: ReactNode; dim?: boolean; children?: ReactNode }) {
  return (
    <div className="flex min-h-14 items-center gap-3 border-b border-rule py-2.5">
      {props.lead}
      <div className="min-w-0 flex-1">
        <div className={cx("truncate font-semibold", props.dim && "font-normal text-muted")}>{props.title}</div>
        {props.detail && <div className="text-caption text-muted">{props.detail}</div>}
      </div>
      {props.children}
    </div>
  );
}

export function LinkRow(props: { title: string; detail?: string; danger?: boolean; onClick: () => void }) {
  return (
    <button
      onClick={props.onClick}
      className={cx(
        "flex min-h-14 w-full items-center gap-3 border-b border-rule py-2.5 text-left transition-colors hover:bg-soft",
        props.danger && "text-error",
      )}
    >
      <div className="min-w-0 flex-1">
        <div className="truncate font-semibold">{props.title}</div>
        {props.detail && <div className={cx("text-caption", props.danger ? "text-error" : "text-muted")}>{props.detail}</div>}
      </div>
      <ChevronRight size={18} className="shrink-0" />
    </button>
  );
}

export function Segmented<T extends string>(props: {
  value: T;
  options: { value: T; label: string }[];
  label: string;
  disabled?: boolean;
  onChange: (value: T) => void;
}) {
  return (
    <div className="flex shrink-0 rounded-[4px] border border-ink" role="radiogroup" aria-label={props.label}>
      {props.options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={props.value === o.value}
          disabled={props.disabled}
          onClick={() => props.onChange(o.value)}
          className={cx(
            "h-7 px-2.5 text-caption transition-colors first:rounded-l-[3px] last:rounded-r-[3px] disabled:opacity-35",
            props.value === o.value ? "bg-ink font-semibold text-bg" : "hover:bg-soft",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Dot(props: { color: "l" | "r" | "none" }) {
  return (
    <span
      className={cx(
        "size-3.5 shrink-0 rounded-full",
        props.color === "l" && "bg-block-l",
        props.color === "r" && "bg-block-r",
        props.color === "none" && "border-[1.5px] border-muted",
      )}
    />
  );
}
