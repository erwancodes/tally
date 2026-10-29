import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { mock } from "./mock";

export type Totals = { input: number; output: number; cache_read: number; cache_write: number };
export type AgentTotals = { agent: string; totals: Totals };
export type Period = { totals: Totals; previous: Totals; by_agent: AgentTotals[] };
export type Summary = { today: Period; week: Period; month: Period };
export type DayStat = { day: string; by_agent: AgentTotals[] };
export type Overview = { summary: Summary; daily: DayStat[]; include_cache: boolean; scanning: boolean };
export type ModelTotals = { agent: string; model: string; totals: Totals; last_seen_ms: number };
export type AgentInfo = {
  id: string;
  name: string;
  enabled: boolean;
  roots: [string, boolean][];
  default_roots: string[];
  custom_path: string | null;
};
export type ProviderSettings = { disabled: boolean; path: string | null };
export type Settings = { providers: Record<string, ProviderSettings>; include_cache: boolean };
export type ScanProgress = { done: number; total: number };

/** Outside Tauri (plain `npm run dev`, screenshots) the app runs on sample data. */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? invoke<T>(cmd, args) : mock<T>(cmd, args);
}

export const api = {
  overview: () => call<Overview>("get_overview"),
  daily: (days: number) => call<DayStat[]>("get_daily", { days }),
  models: () => call<ModelTotals[]>("get_models"),
  agents: () => call<AgentInfo[]>("get_agents"),
  refresh: () => call<void>("refresh"),
  settings: () => call<Settings>("get_settings"),
  setSettings: (settings: Settings) => call<void>("set_settings", { settings }),
};

export function onEvent<T>(name: string, fn: (payload: T) => void): () => void {
  if (!inTauri) return () => {};
  let off: UnlistenFn | undefined;
  let dead = false;
  listen<T>(name, (e) => fn(e.payload)).then((u) => (dead ? u() : (off = u)));
  return () => {
    dead = true;
    off?.();
  };
}

export const sum = (t: Totals, includeCache: boolean) =>
  t.input + t.output + (includeCache ? t.cache_read + t.cache_write : 0);
