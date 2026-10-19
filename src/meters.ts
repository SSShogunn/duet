import { useSyncExternalStore } from "react";
import { listen } from "@tauri-apps/api/event";

export type Meters = {
  spectrum: number[];
  levels: Record<string, number>;
};

let current: Meters = { spectrum: [], levels: {} };
const listeners = new Set<() => void>();

listen<Meters>("meters", (event) => {
  current = event.payload;
  listeners.forEach((notify) => notify());
});

function subscribe(notify: () => void) {
  listeners.add(notify);
  return () => {
    listeners.delete(notify);
  };
}

export function useMeters() {
  return useSyncExternalStore(subscribe, () => current);
}
