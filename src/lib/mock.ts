// Sample data for running the front without Tauri. Never used in the app itself.
import type { AgentInfo, DayStat, ModelTotals, Overview, ProjectTotals, SessionTotals, Settings, Totals } from "./api";

function rand(seed: number) {
  const x = Math.sin(seed * 9301 + 49297) * 233280;
  return x - Math.floor(x);
}

function totals(scale: number, seed: number): Totals {
  const r = 0.6 + rand(seed) * 0.8;
  return {
    input: Math.round(18_000 * scale * r),
    output: Math.round(92_000 * scale * r),
    cache_read: Math.round(1_150_000 * scale * r),
    cache_write: Math.round(140_000 * scale * r),
  };
}

function iso(d: Date) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function days(n: number): DayStat[] {
  const out: DayStat[] = [];
  for (let i = n - 1; i >= 0; i--) {
    const d = new Date();
    d.setDate(d.getDate() - i);
    const weekend = d.getDay() === 0 || d.getDay() === 6 ? 0.45 : 1;
    out.push({
      day: iso(d),
      by_agent: [
        { agent: "claude-code", totals: totals(weekend * 1.1, i) },
        ...(rand(i + 7) > 0.35 ? [{ agent: "codex", totals: totals(weekend * 0.32, i + 50) }] : []),
      ],
    });
  }
  return out;
}

function add(a: Totals, b: Totals): Totals {
  return {
    input: a.input + b.input,
    output: a.output + b.output,
    cache_read: a.cache_read + b.cache_read,
    cache_write: a.cache_write + b.cache_write,
  };
}

const zero: Totals = { input: 0, output: 0, cache_read: 0, cache_write: 0 };

function period(list: DayStat[], prev: DayStat[]) {
  const byAgent = new Map<string, Totals>();
  for (const d of list) for (const a of d.by_agent) byAgent.set(a.agent, add(byAgent.get(a.agent) ?? zero, a.totals));
  const by_agent = [...byAgent].map(([agent, t]) => ({ agent, totals: t }));
  const all = (l: DayStat[]) => l.flatMap((d) => d.by_agent.map((a) => a.totals)).reduce(add, zero);
  return { totals: all(list), previous: all(prev), by_agent };
}

const sixty = days(60);
const last = (n: number, skip = 0) => sixty.slice(sixty.length - n - skip, sixty.length - skip);
const monthDay = new Date().getDate();

const projects = ["tally", "stagepilot", "terratalk", null];
const sessions: SessionTotals[] = Array.from({ length: 14 }, (_, i) => {
  const agent = i % 3 === 2 ? "codex" : "claude-code";
  const last = Date.now() - i * 5_400_000 - rand(i) * 3_600_000;
  return {
    agent,
    session: Math.floor(rand(i + 99) * 0xffffffff).toString(16).padStart(8, "0") + "-4c1a",
    project: projects[i % projects.length],
    models: agent === "codex" ? ["gpt-6-sol"] : i % 4 === 0 ? ["claude-opus-5-5", "claude-haiku-5-5"] : ["claude-opus-5-5"],
    first_ms: last - (20 + rand(i + 3) * 160) * 60_000,
    last_ms: last,
    totals: totals(0.8 + rand(i + 5) * 2, i + 20),
    cost: agent === "codex" ? null : 1.2 + rand(i + 9) * 18,
  };
});

let settings: Settings = { providers: {}, include_cache: true };

const handlers: Record<string, (args?: Record<string, unknown>) => unknown> = {
  get_overview: (): Overview => ({
    summary: {
      today: period(last(1), last(1, 1)),
      week: period(last(7), last(7, 7)),
      month: period(last(monthDay), last(monthDay, 30)),
    },
    daily: last(30),
    include_cache: settings.include_cache,
    scanning: false,
  }),
  get_daily: (args) => last(Number(args?.days ?? 30)),
  get_models: (): ModelTotals[] => [
    { agent: "claude-code", model: "claude-opus-5-5", totals: totals(24, 1), last_seen_ms: Date.now() - 4000, cost: 214.3 },
    { agent: "claude-code", model: "claude-sonnet-5-5", totals: totals(6, 2), last_seen_ms: Date.now() - 86_400_000, cost: 26.8 },
    { agent: "claude-code", model: "claude-haiku-5-5", totals: totals(0.8, 3), last_seen_ms: Date.now() - 3 * 86_400_000, cost: 0.21 },
    { agent: "codex", model: "gpt-6-sol", totals: totals(7, 4), last_seen_ms: Date.now() - 3_600_000, cost: null },
  ],
  get_sessions: (): SessionTotals[] => sessions,
  get_projects: (): ProjectTotals[] => {
    const by = new Map<string | null, ProjectTotals>();
    for (const s of sessions) {
      const p = by.get(s.project) ?? { project: s.project, agents: [], sessions: 0, last_ms: 0, totals: zero, cost: null };
      by.set(s.project, {
        ...p,
        agents: p.agents.includes(s.agent) ? p.agents : [...p.agents, s.agent],
        sessions: p.sessions + 1,
        last_ms: Math.max(p.last_ms, s.last_ms),
        totals: add(p.totals, s.totals),
        cost: s.cost === null ? p.cost : (p.cost ?? 0) + s.cost,
      });
    }
    return [...by.values()].sort((a, b) => b.totals.cache_read - a.totals.cache_read);
  },
  get_agents: (): AgentInfo[] => [
    {
      id: "claude-code",
      name: "Claude Code",
      enabled: !settings.providers["claude-code"]?.disabled,
      roots: [["C:\\Users\\dev\\.claude\\projects", true]],
      default_roots: ["C:\\Users\\dev\\.claude\\projects"],
      custom_path: null,
    },
    {
      id: "codex",
      name: "Codex",
      enabled: !settings.providers.codex?.disabled,
      roots: [["C:\\Users\\dev\\.codex\\sessions", true]],
      default_roots: ["C:\\Users\\dev\\.codex\\sessions"],
      custom_path: null,
    },
  ],
  get_settings: () => settings,
  set_settings: (args) => {
    settings = args?.settings as Settings;
  },
  refresh: () => undefined,
  get_update: () => null,
};

export async function mock<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return handlers[cmd]?.(args) as T;
}
