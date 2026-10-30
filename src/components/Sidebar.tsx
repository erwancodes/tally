import { useEffect, useState } from "react";
import { api, onEvent } from "../lib/api";

export type View = "overview" | "agents" | "sessions" | "history" | "settings";

const ITEMS: { id: View; label: string; icon: React.ReactNode }[] = [
  {
    id: "overview",
    label: "Overview",
    icon: <path d="M2 13V8M6 13V3M10 13V6M14 13V10" />,
  },
  {
    id: "agents",
    label: "By agent",
    icon: (
      <>
        <circle cx="5" cy="6" r="2.5" />
        <circle cx="11" cy="6" r="2.5" />
        <path d="M1.5 13c.5-2 2-3 3.5-3s3 1 3.5 3M7.5 13c.5-2 2-3 3.5-3s3 1 3.5 3" />
      </>
    ),
  },
  {
    id: "sessions",
    label: "Sessions",
    icon: (
      <>
        <rect x="2" y="2.5" width="12" height="11" rx="2" />
        <path d="M2 6h12M5 9h3M5 11h5" />
      </>
    ),
  },
  {
    id: "history",
    label: "History",
    icon: (
      <>
        <circle cx="8" cy="8" r="6" />
        <path d="M8 4.5V8l2.5 1.5" />
      </>
    ),
  },
  {
    id: "settings",
    label: "Settings",
    icon: (
      <>
        <circle cx="8" cy="8" r="2" />
        <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
      </>
    ),
  },
];

export function Sidebar({ view, onChange }: { view: View; onChange: (v: View) => void }) {
  return (
    <nav className="flex w-52 shrink-0 flex-col gap-0.5 px-3 pt-4">
      {ITEMS.map((item) => {
        const active = item.id === view;
        return (
          <button
            key={item.id}
            onClick={() => onChange(item.id)}
            aria-current={active ? "page" : undefined}
            className={`flex h-9 items-center gap-3 rounded-lg px-3 text-left text-[13.5px] transition-colors active:scale-[0.98] ${
              active ? "bg-panel-2 text-ink" : "text-ink-2 hover:bg-panel hover:text-ink"
            }`}
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
              {item.icon}
            </svg>
            {item.label}
          </button>
        );
      })}
      <UpdateCard />
    </nav>
  );
}

/** Shows up only when a newer signed release is on GitHub. */
function UpdateCard() {
  const [version, setVersion] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);

  useEffect(() => {
    api.update().then((v) => v && setVersion(v));
    return onEvent<string>("update-available", setVersion);
  }, []);

  if (!version) return null;
  const install = () => {
    setBusy(true);
    setError(false);
    api.installUpdate().catch(() => {
      setBusy(false);
      setError(true);
    });
  };
  return (
    <div className="mt-auto mb-4 rounded-lg bg-panel px-3 py-3 text-[12.5px]">
      <p className="text-ink">Tally {version} is out</p>
      <p className="mt-0.5 text-ink-3">{error ? "Update failed. Try again." : "Installs and restarts in a few seconds."}</p>
      <button
        onClick={install}
        disabled={busy}
        className="mt-2.5 h-7 w-full rounded-md bg-ink text-[12.5px] font-medium text-ground transition-opacity hover:opacity-90 active:scale-[0.98] disabled:opacity-60"
      >
        {busy ? "Updating…" : "Update now"}
      </button>
    </div>
  );
}
