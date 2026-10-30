import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { api, inTauri, type AgentInfo, type Settings } from "../lib/api";
import { agentColor } from "../lib/agents";

export function SettingsView({ onChanged }: { onChanged: () => void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [autostart, setAutostart] = useState(false);

  const load = () => {
    api.settings().then(setSettings);
    api.agents().then(setAgents);
  };

  useEffect(() => {
    load();
    if (inTauri) isEnabled().then(setAutostart);
  }, []);

  if (!settings) return null;

  const save = async (next: Settings) => {
    setSettings(next);
    await api.setSettings(next);
    api.agents().then(setAgents);
    onChanged();
  };

  const provider = (id: string) => settings.providers[id] ?? { disabled: false, path: null };
  const setProvider = (id: string, patch: Partial<{ disabled: boolean; path: string | null }>) =>
    save({ ...settings, providers: { ...settings.providers, [id]: { ...provider(id), ...patch } } });

  return (
    <div className="flex max-w-2xl flex-col gap-6">
      <h1 className="text-[22px] font-semibold tracking-tight">Settings</h1>

      <section className="rise flex flex-col gap-1 rounded-2xl bg-panel p-2">
        <Row title="Launch at startup" detail="Tally starts minimized in the system tray.">
          <Toggle
            label="Launch at startup"
            on={autostart}
            onChange={async (v) => {
              if (!inTauri) return setAutostart(v);
              await (v ? enable() : disable());
              setAutostart(await isEnabled());
            }}
          />
        </Row>
        <Row title="Count cache" detail="Include cache read and write tokens in totals.">
          <Toggle label="Count cache" on={settings.include_cache} onChange={(v) => save({ ...settings, include_cache: v })} />
        </Row>
      </section>

      <h2 className="text-[15px] font-medium">Agents</h2>
      {agents.map((a, i) => (
        <AgentCard
          key={a.id}
          agent={a}
          delay={60 * (i + 1)}
          onToggle={(on) => setProvider(a.id, { disabled: !on })}
          onPath={(path) => setProvider(a.id, { path: path.trim() || null })}
        />
      ))}

      <section className="rise flex items-center justify-between rounded-2xl bg-panel p-4">
        <div>
          <div className="text-[13.5px]">Recount everything</div>
          <div className="text-[12.5px] text-ink-3">Clears the database and rereads every file.</div>
        </div>
        <button
          onClick={() => api.refresh().then(onChanged)}
          className="rounded-lg bg-panel-2 px-3.5 py-1.5 text-[13px] transition-colors hover:bg-panel-3 active:scale-[0.98]"
        >
          Refresh
        </button>
      </section>
    </div>
  );
}

function AgentCard({ agent, delay, onToggle, onPath }: { agent: AgentInfo; delay: number; onToggle: (on: boolean) => void; onPath: (p: string) => void }) {
  const [path, setPath] = useState(agent.custom_path ?? "");
  useEffect(() => setPath(agent.custom_path ?? ""), [agent.custom_path]);
  const found = agent.roots.some(([, ok]) => ok);

  return (
    <section className="rise flex flex-col gap-3 rounded-2xl bg-panel p-4" style={{ animationDelay: `${delay}ms` }}>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2.5">
          <span className="size-2.5 rounded-full" style={{ background: agentColor(agent.id) }} />
          <span className="text-[14px] font-medium">{agent.name}</span>
          <span className={`rounded-md px-1.5 py-0.5 text-[12px] ${found ? "bg-up/12 text-up" : "bg-panel-3 text-ink-3"}`}>{found ? "Detected" : "Not found"}</span>
        </div>
        <Toggle label={`Track ${agent.name}`} on={agent.enabled} onChange={onToggle} />
      </div>
      <label className="flex flex-col gap-1.5">
        <span className="text-[12.5px] text-ink-2">Dossier</span>
        <input
          value={path}
          onChange={(e) => setPath(e.target.value)}
          onBlur={() => path !== (agent.custom_path ?? "") && onPath(path)}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          placeholder={agent.default_roots.join(" ; ")}
          spellCheck={false}
          className="h-9 rounded-lg bg-panel-2 px-3 text-[13px] text-ink outline-none placeholder:text-ink-3 focus:bg-panel-3"
        />
      </label>
    </section>
  );
}

function Row({ title, detail, children }: { title: string; detail: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 rounded-xl px-3 py-3 odd:bg-panel-2/40">
      <div>
        <div className="text-[13.5px]">{title}</div>
        <div className="text-[12.5px] text-ink-3">{detail}</div>
      </div>
      {children}
    </div>
  );
}

function Toggle({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
      className={`relative h-5.5 w-9.5 shrink-0 rounded-full transition-colors ${on ? "bg-claude" : "bg-panel-3"}`}
    >
      <span className={`absolute top-0.75 left-0.75 size-4 rounded-full bg-white transition-transform duration-200 ${on ? "translate-x-4" : ""}`} />
    </button>
  );
}
