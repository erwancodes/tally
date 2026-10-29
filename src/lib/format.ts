const NNBSP = " ";

/** French short format: `1 234` under 10k, then `12,3k`, `1,24M`, `1,2B`. Same rules as `format.rs`. */
export function compact(n: number): string {
  if (n < 10_000) return Math.round(n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, NNBSP);
  if (n < 1_000_000) return `${(n / 1e3).toFixed(1).replace(".", ",")}k`;
  if (n < 1_000_000_000) return `${(n / 1e6).toFixed(2).replace(".", ",")}M`;
  return `${(n / 1e9).toFixed(1).replace(".", ",")}B`;
}

/** Change against the previous period, or null when there is nothing to compare with. */
export function variation(current: number, previous: number): number | null {
  if (previous <= 0) return null;
  return (current - previous) / previous;
}

export function percent(v: number): string {
  const p = Math.round(v * 100);
  return `${p > 0 ? "+" : ""}${p}${NNBSP}%`;
}

export function ago(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 60) return `il y a ${s}${NNBSP}s`;
  const m = Math.round(s / 60);
  if (m < 60) return `il y a ${m}${NNBSP}min`;
  return `il y a ${Math.round(m / 60)}${NNBSP}h`;
}

const dayFmt = new Intl.DateTimeFormat("fr-FR", { weekday: "short", day: "numeric", month: "short" });
const shortFmt = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short" });

/** `2026-10-29` read as a local date. */
export function parseDay(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
}

export const formatDay = (day: string) => dayFmt.format(parseDay(day));
export const formatShortDay = (day: string) => shortFmt.format(parseDay(day));
