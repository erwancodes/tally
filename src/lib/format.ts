/** Short format: `1,234` under 10k, then `12.3k`, `1.24M`, `1.2B`. Same rules as `format.rs`. */
export function compact(n: number): string {
  if (n < 10_000) return Math.round(n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  if (n < 1_000_000) return `${(n / 1e3).toFixed(1)}k`;
  if (n < 1_000_000_000) return `${(n / 1e6).toFixed(2)}M`;
  return `${(n / 1e9).toFixed(1)}B`;
}

/** Change against the previous period, or null when there is nothing to compare with. */
export function variation(current: number, previous: number): number | null {
  if (previous <= 0) return null;
  return (current - previous) / previous;
}

export function percent(v: number): string {
  const p = Math.round(v * 100);
  return `${p > 0 ? "+" : ""}${p}%`;
}

/** Estimated cost: `$0.42`, `$12.40`, `$1,240`. */
export function money(n: number): string {
  if (n < 0.01) return "<$0.01";
  if (n < 100) return `$${n.toFixed(2)}`;
  return `$${Math.round(n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",")}`;
}

/** `45min`, `3h 20min`, `2d 4h`. */
export function duration(ms: number): string {
  const m = Math.max(1, Math.round(ms / 60_000));
  if (m < 60) return `${m}min`;
  const h = Math.floor(m / 60);
  if (h < 24) return m % 60 ? `${h}h ${m % 60}min` : `${h}h`;
  return h % 24 ? `${Math.floor(h / 24)}d ${h % 24}h` : `${Math.floor(h / 24)}d`;
}

export function ago(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 60) return `${s}s ago`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m}min ago`;
  const h = Math.round(m / 60);
  if (h < 48) return `${h}h ago`;
  return `${Math.round(h / 24)}d ago`;
}

const dayFmt = new Intl.DateTimeFormat("en-US", { weekday: "short", day: "numeric", month: "short" });
const shortFmt = new Intl.DateTimeFormat("en-US", { day: "numeric", month: "short" });

/** `2026-10-29` read as a local date. */
export function parseDay(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
}

export const formatDay = (day: string) => dayFmt.format(parseDay(day));
export const formatShortDay = (day: string) => shortFmt.format(parseDay(day));
