import { useEffect, useRef, useState } from "react";
import { Bar, BarChart, Cell, Pie, PieChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { sum, type Overview as OverviewData, type Period } from "../lib/api";
import { agentColor, agentName } from "../lib/agents";
import { ago, compact, formatDay, formatShortDay, percent, variation } from "../lib/format";
import type { OverviewState } from "../lib/useOverview";

export function Overview({ data, onSettings }: { data: OverviewState; onSettings: () => void }) {
  const { overview, updatedAt, progress } = data;
  if (!overview) return <Scanning progress={progress} />;

  const cache = overview.include_cache;
  const empty = overview.summary.month.totals.input + overview.summary.month.totals.output === 0 && overview.daily.every((d) => d.by_agent.length === 0);
  if (empty && !overview.scanning) return <Empty onSettings={onSettings} />;

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-baseline justify-between">
        <h1 className="text-[22px] font-semibold tracking-tight">Vue d'ensemble</h1>
        <Updated at={updatedAt} progress={progress} />
      </div>

      <div className="grid grid-cols-3 gap-3">
        <StatCard label="Today" hint="vs yesterday at the same time" period={overview.summary.today} cache={cache} delay={0} />
        <StatCard label="Last 7 days" hint="vs the previous 7" period={overview.summary.week} cache={cache} delay={60} />
        <StatCard label="This month" hint="vs last month" period={overview.summary.month} cache={cache} delay={120} />
      </div>

      <section className="rise rounded-2xl bg-panel p-5" style={{ animationDelay: "180ms" }}>
        <div className="mb-4 flex items-baseline justify-between">
          <h2 className="text-[15px] font-medium">30 derniers jours</h2>
          <Legend agents={agentsOf(overview)} />
        </div>
        <DailyBars overview={overview} />
      </section>

      <div className="grid grid-cols-[1fr_1.4fr] gap-3">
        <section className="rise rounded-2xl bg-panel p-5" style={{ animationDelay: "240ms" }}>
          <h2 className="mb-2 text-[15px] font-medium">This month by agent</h2>
          <Donut period={overview.summary.month} cache={cache} />
        </section>
        <section className="rise rounded-2xl bg-panel p-5" style={{ animationDelay: "300ms" }}>
          <h2 className="mb-4 text-[15px] font-medium">This month breakdown</h2>
          <Split period={overview.summary.month} cache={cache} />
        </section>
      </div>
    </div>
  );
}

export function agentsOf(o: OverviewData) {
  const set = new Set<string>();
  for (const d of o.daily) for (const a of d.by_agent) set.add(a.agent);
  for (const a of o.summary.month.by_agent) set.add(a.agent);
  return [...set].sort();
}

function useCountUp(target: number, delay = 0) {
  const [value, setValue] = useState(0);
  const from = useRef(0);
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setValue(target);
      return;
    }
    const start = performance.now() + delay;
    const origin = from.current;
    let frame = 0;
    const tick = (now: number) => {
      const t = Math.min(1, Math.max(0, (now - start) / 700));
      const eased = 1 - Math.pow(1 - t, 3);
      const v = origin + (target - origin) * eased;
      from.current = v;
      setValue(v);
      if (t < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [target, delay]);
  return value;
}

function StatCard({ label, hint, period, cache, delay }: { label: string; hint: string; period: Period; cache: boolean; delay: number }) {
  const total = sum(period.totals, cache);
  const shown = useCountUp(total, delay);
  const v = variation(total, sum(period.previous, cache));
  return (
    <section className="rise rounded-2xl bg-panel p-5" style={{ animationDelay: `${delay}ms` }}>
      <h2 className="text-[13px] text-ink-2">{label}</h2>
      <div className="num mt-2 text-[32px] leading-none font-semibold tracking-tight">{compact(shown)}</div>
      <div className="mt-3 flex items-center gap-2 text-[12.5px]">
        {v === null ? (
          <span className="text-ink-3">Nothing to compare</span>
        ) : (
          <>
            <span className={`num rounded-md px-1.5 py-0.5 ${v >= 0 ? "bg-up/12 text-up" : "bg-down/12 text-down"}`}>{percent(v)}</span>
            <span className="text-ink-3">{hint}</span>
          </>
        )}
      </div>
    </section>
  );
}

function Legend({ agents }: { agents: string[] }) {
  return (
    <div className="flex gap-4 text-[12.5px] text-ink-2">
      {agents.map((a) => (
        <span key={a} className="flex items-center gap-1.5">
          <span className="size-2 rounded-full" style={{ background: agentColor(a) }} />
          {agentName(a)}
        </span>
      ))}
    </div>
  );
}

function ChartTooltip({ active, payload, label }: { active?: boolean; payload?: { dataKey: string; value: number }[]; label?: string }) {
  if (!active || !payload?.length || !label) return null;
  const rows = payload.filter((p) => p.value > 0);
  return (
    <div className="rounded-lg bg-panel-3 px-3 py-2 text-[12.5px]">
      <div className="mb-1 text-ink-2 capitalize">{formatDay(label)}</div>
      {rows.length === 0 && <div className="text-ink-3">No activity</div>}
      {rows.map((p) => (
        <div key={p.dataKey} className="flex items-center justify-between gap-6">
          <span className="flex items-center gap-1.5">
            <span className="size-2 rounded-full" style={{ background: agentColor(p.dataKey) }} />
            {agentName(p.dataKey)}
          </span>
          <span className="num">{compact(p.value)}</span>
        </div>
      ))}
    </div>
  );
}

function DailyBars({ overview }: { overview: OverviewData }) {
  const agents = agentsOf(overview);
  const rows = overview.daily.map((d) => {
    const row: Record<string, number | string> = { day: d.day };
    for (const a of agents) row[a] = 0;
    for (const a of d.by_agent) row[a.agent] = sum(a.totals, overview.include_cache);
    return row;
  });
  return (
    <div className="h-52">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={rows} barCategoryGap="22%" margin={{ top: 4, right: 0, bottom: 0, left: 0 }}>
          <XAxis
            dataKey="day"
            tickFormatter={formatShortDay}
            interval={6}
            tick={{ fill: "var(--ink-3)", fontSize: 11.5 }}
            axisLine={false}
            tickLine={false}
          />
          <YAxis tickFormatter={compact} width={48} tick={{ fill: "var(--ink-3)", fontSize: 11.5 }} axisLine={false} tickLine={false} />
          <Tooltip content={<ChartTooltip />} cursor={{ fill: "var(--panel-2)" }} isAnimationActive={false} />
          {agents.map((a, i) => (
            <Bar
              key={a}
              dataKey={a}
              stackId="t"
              fill={agentColor(a)}
              radius={i === agents.length - 1 ? [3, 3, 0, 0] : 0}
              animationDuration={600}
            />
          ))}
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

function Donut({ period, cache }: { period: Period; cache: boolean }) {
  const rows = period.by_agent.map((a) => ({ agent: a.agent, value: sum(a.totals, cache) })).filter((r) => r.value > 0);
  const total = rows.reduce((s, r) => s + r.value, 0);
  return (
    <div className="flex items-center gap-5">
      <div className="relative size-36 shrink-0">
        <ResponsiveContainer width="100%" height="100%">
          <PieChart>
            <Pie data={rows} dataKey="value" innerRadius="68%" outerRadius="100%" stroke="none" paddingAngle={rows.length > 1 ? 2 : 0} animationDuration={700}>
              {rows.map((r) => (
                <Cell key={r.agent} fill={agentColor(r.agent)} />
              ))}
            </Pie>
          </PieChart>
        </ResponsiveContainer>
        <div className="num absolute inset-0 grid place-items-center text-[17px] font-semibold">{compact(total)}</div>
      </div>
      <ul className="flex flex-1 flex-col gap-2.5 text-[13px]">
        {rows.map((r) => (
          <li key={r.agent} className="flex items-center justify-between gap-3">
            <span className="flex items-center gap-2">
              <span className="size-2 rounded-full" style={{ background: agentColor(r.agent) }} />
              {agentName(r.agent)}
            </span>
            <span className="num text-ink-2">{total ? Math.round((r.value / total) * 100) : 0}%</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Split({ period, cache }: { period: Period; cache: boolean }) {
  const t = period.totals;
  const parts = [
    { label: "Input", value: t.input, color: "var(--claude)" },
    { label: "Output", value: t.output, color: "var(--codex)" },
    ...(cache
      ? [
          { label: "Cache read", value: t.cache_read, color: "#5f8f71" },
          { label: "Cache write", value: t.cache_write, color: "#7468a8" },
        ]
      : []),
  ];
  const total = parts.reduce((s, p) => s + p.value, 0) || 1;
  return (
    <div className="flex flex-col gap-4">
      <div className="flex h-3 gap-0.5 overflow-hidden rounded-full">
        {parts.map((p) => (
          <div key={p.label} className="h-full transition-[flex-grow] duration-500" style={{ flexGrow: p.value, background: p.color, minWidth: p.value ? 3 : 0 }} />
        ))}
      </div>
      <div className="grid grid-cols-2 gap-x-6 gap-y-3 text-[13px]">
        {parts.map((p) => (
          <div key={p.label} className="flex items-center justify-between gap-3">
            <span className="flex items-center gap-2 text-ink-2">
              <span className="size-2 rounded-full" style={{ background: p.color }} />
              {p.label}
            </span>
            <span className="num">
              {compact(p.value)} <span className="text-ink-3">· {Math.round((p.value / total) * 100)}%</span>
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function Updated({ at, progress }: { at: number; progress: OverviewState["progress"] }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  if (progress && progress.done < progress.total)
    return (
      <span className="num text-[12.5px] text-ink-2">
        Analyse {progress.done}/{progress.total} fichiers
      </span>
    );
  return <span className="text-[12.5px] text-ink-3">Updated {ago(at, now)}</span>;
}

function Scanning({ progress }: { progress: OverviewState["progress"] }) {
  return (
    <div className="grid h-full place-items-center text-ink-2">
      {progress ? `Scanning ${progress.done}/${progress.total} files…` : "Loading…"}
    </div>
  );
}

function Empty({ onSettings }: { onSettings: () => void }) {
  return (
    <div className="grid h-full place-items-center">
      <div className="flex flex-col items-center gap-4 text-center">
        <p className="text-[15px] text-ink-2">No agent detected. Check the paths in ⚙.</p>
        <button onClick={onSettings} className="rounded-lg bg-panel-2 px-4 py-2 text-[13px] transition-colors hover:bg-panel-3 active:scale-[0.98]">
          Open settings
        </button>
      </div>
    </div>
  );
}
