import { useEffect, useState } from "react";
import { api, sum, type ModelTotals } from "../lib/api";
import { agentColor, agentName } from "../lib/agents";
import { ago, compact } from "../lib/format";
import type { OverviewState } from "../lib/useOverview";

export function Agents({ data }: { data: OverviewState }) {
  const [models, setModels] = useState<ModelTotals[] | null>(null);
  const cache = data.overview?.include_cache ?? true;

  useEffect(() => {
    api.models().then(setModels);
  }, [data.updatedAt]);

  if (!models) return null;
  const byAgent = new Map<string, ModelTotals[]>();
  for (const m of models) byAgent.set(m.agent, [...(byAgent.get(m.agent) ?? []), m]);
  const month = new Map(data.overview?.summary.month.by_agent.map((a) => [a.agent, sum(a.totals, cache)]));

  return (
    <div className="flex flex-col gap-6">
      <h1 className="text-[22px] font-semibold tracking-tight">By agent</h1>
      {byAgent.size === 0 && <p className="text-ink-2">No data yet.</p>}
      {[...byAgent].map(([agent, list], i) => {
        const total = list.reduce((s, m) => s + sum(m.totals, cache), 0);
        const sorted = [...list].sort((a, b) => sum(b.totals, cache) - sum(a.totals, cache));
        return (
          <section key={agent} className="rise rounded-2xl bg-panel p-5" style={{ animationDelay: `${i * 60}ms` }}>
            <div className="mb-4 flex items-baseline justify-between">
              <h2 className="flex items-center gap-2 text-[15px] font-medium">
                <span className="size-2.5 rounded-full" style={{ background: agentColor(agent) }} />
                {agentName(agent)}
              </h2>
              <div className="num text-[13px] text-ink-2">
                {compact(month.get(agent) ?? 0)} this month · {compact(total)} all time
              </div>
            </div>
            <table className="w-full text-[13px]">
              <thead className="text-left text-ink-3">
                <tr>
                  <th className="pb-2 font-normal">Model</th>
                  <th className="pb-2 text-right font-normal">Input</th>
                  <th className="pb-2 text-right font-normal">Output</th>
                  {cache && <th className="pb-2 text-right font-normal">Cache</th>}
                  <th className="pb-2 text-right font-normal">Total</th>
                  <th className="pb-2 text-right font-normal">Last used</th>
                </tr>
              </thead>
              <tbody className="num">
                {sorted.map((m) => {
                  const t = sum(m.totals, cache);
                  return (
                    <tr key={m.model} className="odd:bg-panel-2/50">
                      <td className="relative rounded-l-lg py-2 pl-2">
                        <div className="absolute inset-y-1.5 left-0 rounded-full opacity-20" style={{ width: `${(t / (total || 1)) * 100}%`, background: agentColor(agent) }} />
                        <span className="relative">{m.model}</span>
                      </td>
                      <td className="py-2 text-right">{compact(m.totals.input)}</td>
                      <td className="py-2 text-right">{compact(m.totals.output)}</td>
                      {cache && <td className="py-2 text-right">{compact(m.totals.cache_read + m.totals.cache_write)}</td>}
                      <td className="py-2 text-right font-medium">{compact(t)}</td>
                      <td className="rounded-r-lg py-2 pr-2 text-right text-ink-2">{ago(m.last_seen_ms)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </section>
        );
      })}
    </div>
  );
}
