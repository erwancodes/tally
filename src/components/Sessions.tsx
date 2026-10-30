import { useEffect, useState } from "react";
import { api, sum, type ProjectTotals, type SessionTotals } from "../lib/api";
import { agentColor, agentName } from "../lib/agents";
import { ago, compact, duration, money } from "../lib/format";
import type { OverviewState } from "../lib/useOverview";
import { Segmented } from "./History";

type Mode = "sessions" | "projects";

const RANGES = [1, 7, 30, 90];

export function Sessions({ data }: { data: OverviewState }) {
  const [mode, setMode] = useState<Mode>("sessions");
  const [days, setDays] = useState(7);
  const [sessions, setSessions] = useState<SessionTotals[] | null>(null);
  const [projects, setProjects] = useState<ProjectTotals[] | null>(null);
  const cache = data.overview?.include_cache ?? true;

  useEffect(() => {
    if (mode === "sessions") api.sessions(days, 200).then(setSessions);
    else api.projects(days).then(setProjects);
  }, [mode, days, data.updatedAt]);

  const rows = mode === "sessions" ? sessions : projects;
  const cost = rows?.reduce((s, r) => s + (r.cost ?? 0), 0) ?? 0;
  const unpriced = rows?.some((r) => r.cost === null) ?? false;

  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-center justify-between">
        <h1 className="text-[22px] font-semibold tracking-tight">Sessions</h1>
        <div className="flex gap-2">
          <Segmented<Mode>
            options={[
              { id: "sessions", label: "Sessions" },
              { id: "projects", label: "Projects" },
            ]}
            value={mode}
            onChange={setMode}
          />
          <Segmented options={RANGES.map((r) => ({ id: r, label: r === 1 ? "Today" : `${r}d` }))} value={days} onChange={setDays} />
        </div>
      </div>
      {rows && rows.length > 0 && (
        <p className="-mt-2 text-[13px] text-ink-2">
          <span className="num font-medium text-ink">≈ {money(cost)}</span> at API list prices
          {unpriced && ", models without a known price left out"}. A subscription bills differently.
        </p>
      )}
      <div className="rise rounded-2xl bg-panel p-2">
        {rows && rows.length === 0 && <p className="p-4 text-ink-2">No activity in this period.</p>}
        {mode === "sessions" && sessions && sessions.length > 0 && <SessionTable rows={sessions} cache={cache} />}
        {mode === "projects" && projects && projects.length > 0 && <ProjectTable rows={projects} cache={cache} />}
      </div>
    </div>
  );
}

function Cost({ value }: { value: number | null }) {
  return value === null ? <span className="text-ink-3">n/a</span> : <>{money(value)}</>;
}

function SessionTable({ rows, cache }: { rows: SessionTotals[]; cache: boolean }) {
  return (
    <table className="w-full text-[13px]">
      <thead className="text-left text-ink-3">
        <tr>
          <th className="px-3 py-2 font-normal">Session</th>
          <th className="py-2 font-normal">Model</th>
          <th className="py-2 text-right font-normal">Duration</th>
          <th className="py-2 text-right font-normal">Last active</th>
          <th className="py-2 text-right font-normal">Tokens</th>
          <th className="px-3 py-2 text-right font-normal">Est. cost</th>
        </tr>
      </thead>
      <tbody className="num">
        {rows.map((s) => (
          <tr key={`${s.agent}:${s.session}`} className="odd:bg-panel-2/50">
            <td className="rounded-l-lg px-3 py-2">
              <span className="flex items-center gap-2" title={`${agentName(s.agent)} · ${s.session}`}>
                <span className="size-2 shrink-0 rounded-full" style={{ background: agentColor(s.agent) }} />
                <span className="truncate">{s.project ?? "No project"}</span>
                <span className="text-ink-3">{s.session.slice(0, 8)}</span>
              </span>
            </td>
            <td className="py-2 text-ink-2">
              {s.models[0] ?? "Unknown"}
              {s.models.length > 1 && <span className="text-ink-3"> +{s.models.length - 1}</span>}
            </td>
            <td className="py-2 text-right text-ink-2">{duration(s.last_ms - s.first_ms)}</td>
            <td className="py-2 text-right text-ink-2">{ago(s.last_ms)}</td>
            <td className="py-2 text-right">{compact(sum(s.totals, cache))}</td>
            <td className="rounded-r-lg px-3 py-2 text-right font-medium">
              <Cost value={s.cost} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function ProjectTable({ rows, cache }: { rows: ProjectTotals[]; cache: boolean }) {
  const top = Math.max(1, ...rows.map((p) => sum(p.totals, cache)));
  return (
    <table className="w-full text-[13px]">
      <thead className="text-left text-ink-3">
        <tr>
          <th className="px-3 py-2 font-normal">Project</th>
          <th className="py-2 text-right font-normal">Sessions</th>
          <th className="py-2 text-right font-normal">Last active</th>
          <th className="py-2 text-right font-normal">Tokens</th>
          <th className="px-3 py-2 text-right font-normal">Est. cost</th>
        </tr>
      </thead>
      <tbody className="num">
        {rows.map((p) => {
          const t = sum(p.totals, cache);
          return (
            <tr key={p.project ?? ""} className="odd:bg-panel-2/50">
              <td className="relative rounded-l-lg px-3 py-2">
                <div className="absolute inset-y-1.5 left-0 rounded-full bg-ink opacity-[0.07]" style={{ width: `${(t / top) * 100}%` }} />
                <span className="relative flex items-center gap-2">
                  <span className="flex gap-1">
                    {p.agents.map((a) => (
                      <span key={a} title={agentName(a)} className="size-2 rounded-full" style={{ background: agentColor(a) }} />
                    ))}
                  </span>
                  {p.project ?? "No project"}
                </span>
              </td>
              <td className="py-2 text-right text-ink-2">{p.sessions}</td>
              <td className="py-2 text-right text-ink-2">{ago(p.last_ms)}</td>
              <td className="py-2 text-right">{compact(t)}</td>
              <td className="rounded-r-lg px-3 py-2 text-right font-medium">
                <Cost value={p.cost} />
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
