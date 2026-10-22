export type DeviceView = {
  id: string;
  name: string;
  isDefault: boolean;
  isVirtual: boolean;
  enabled: boolean;
  offsetMs: number;
  side: "L" | "R" | null;
};

export type StateView = {
  status: string;
  running: boolean;
  syncing: boolean;
  virtualSelected: boolean;
  virtualName: string | null;
  devices: DeviceView[];
  stereo: boolean;
  swap: boolean;
  autoSync: boolean;
  syncSound: SyncSound;
  backgroundSyncMin: number;
  unstableSpeaker: string | null;
  bluetoothResetting: boolean;
  cableRenamed: boolean;
  cablePrompt: boolean;
  lastSync: string | null;
};

export type SyncSound = "silent" | "chime";

export type Theme = "system" | "light" | "dark";

export type AppInfo = {
  version: string;
  debugLogging: boolean;
  logDir: string;
  theme: Theme;
};

export type Tone = "ok" | "busy" | "warn" | "idle";

export type Args = Record<string, unknown>;
export type Call = <T>(command: string, args?: Args) => Promise<T | null>;
export type Change = (command: string, args?: Args) => Promise<void>;
