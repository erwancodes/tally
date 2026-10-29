import { useEffect, useState } from "react";
import { api, sum, type DayStat } from "../lib/api";
import { agentColor, agentName } from "../lib/agents";
import { compact, formatDay } from "../lib/format";
import type { OverviewState } from "../lib/useOverview";

const RANGES = [30, 90, 365];

export function History({ data }: { data: OverviewState }) {
  const [days, setDays] = useState(30);
  const [rows, setRows] = useState<DayStat[]>([]);
  const [filter, setFilter] = useState<string | null>(null);
  const cache = data.overview?.include_cache ?? true;

  useEffect(() => {
    api.daily(days).then(setRows);
  }, [days, data.updatedAt]);

  const agents = [...new Set(rows.flatMap((d) => d.by_agent.map((a) => a.agent)))].sort();
  const list = [...rows]
    .reverse()
    .map((d) => ({ day: d.day, by_agent: d.by_agent.filter((a) => !filter || a.agent === filter) }))
    .filter((d) => d.by_agent.length > 0);

  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-center justify-between">
        <h1 className="text-[22px] font-semibold tracking-tight">Historique</h1>
        <div className="flex gap-2">
          <Segmented options={[{ id: null, label: "Tous" }, ...agents.map((a) => ({ id: a, label: agentName(a) }))]} value={filter} onChange={setFilter} />
          <Segmented options={RANGES.map((r) => ({ id: r, label: `${r} j` }))} value={days} onChange={setDays} />
        </div>
      </div>
      <div className="rise rounded-2xl bg-panel p-2">
        {list.length === 0 && <p className="p-4 text-ink-2">Aucune activité sur cette période.</p>}
        <table className="w-full text-[13px]">
          {list.length > 0 && (
            <thead className="text-left text-ink-3">
              <tr>
                <th className="px-3 py-2 font-normal">Jour</th>
                <th className="py-2 font-normal">Agents</th>
                <th className="py-2 text-right font-normal">Input</th>
                <th className="py-2 text-right font-normal">Output</th>
                {cache && <th className="py-2 text-right font-normal">Cache</th>}
                <th className="px-3 py-2 text-right font-normal">Total</th>
              </tr>
            </thead>
          )}
          <tbody className="num">
            {list.map((d) => {
              const t = d.by_agent.reduce(
                (acc, a) => ({
                  input: acc.input + a.totals.input,
                  output: acc.output + a.totals.output,
                  cache_read: acc.cache_read + a.totals.cache_read,
                  cache_write: acc.cache_write + a.totals.cache_write,
                }),
                { input: 0, output: 0, cache_read: 0, cache_write: 0 },
              );
              return (
                <tr key={d.day} className="odd:bg-panel-2/50">
                  <td className="rounded-l-lg px-3 py-2 capitalize">{formatDay(d.day)}</td>
                  <td className="py-2">
                    <span className="flex gap-1">
                      {d.by_agent.map((a) => (
                        <span key={a.agent} title={agentName(a.agent)} className="size-2 rounded-full" style={{ background: agentColor(a.agent) }} />
                      ))}
                    </span>
                  </td>
                  <td className="py-2 text-right">{compact(t.input)}</td>
                  <td className="py-2 text-right">{compact(t.output)}</td>
                  {cache && <td className="py-2 text-right">{compact(t.cache_read + t.cache_write)}</td>}
                  <td className="rounded-r-lg px-3 py-2 text-right font-medium">{compact(sum(t, cache))}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function Segmented<T extends string | number | null>({
  options,
  value,
  onChange,
}: {
  options: { id: T; label: string }[];
  value: T;
  onChange: (v: T) => void;
}) {
  return (
    <div className="flex rounded-lg bg-panel p-0.5 text-[12.5px]">
      {options.map((o) => (
        <button
          key={String(o.id)}
          onClick={() => onChange(o.id)}
          className={`rounded-md px-2.5 py-1 transition-colors active:scale-[0.97] ${o.id === value ? "bg-panel-3 text-ink" : "text-ink-2 hover:text-ink"}`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
