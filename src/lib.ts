import type { Theme, Tone } from "./types";

export function cx(...classes: (string | false | null | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

export function shortName(name: string) {
  return name.replace(/^Speakers \(/, "").replace(/\)$/, "");
}

export const toneClass: Record<Tone, string> = {
  ok: "bg-ok-soft text-ok",
  busy: "bg-busy-soft text-busy",
  warn: "bg-warn-soft text-warn",
  idle: "bg-soft text-muted",
};

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
let chosenTheme: Theme = "system";

function paint() {
  const dark = chosenTheme === "dark" || (chosenTheme === "system" && darkQuery.matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

darkQuery.addEventListener("change", paint);

export function applyTheme(theme: Theme) {
  chosenTheme = theme;
  paint();
}
