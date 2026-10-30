import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri } from "../lib/api";

const win = () => getCurrentWindow();

function Button({ label, onClick, children, danger }: { label: string; onClick: () => void; children: React.ReactNode; danger?: boolean }) {
  return (
    <button
      aria-label={label}
      onClick={onClick}
      className={`grid h-10 w-12 place-items-center text-ink-2 transition-colors hover:text-ink ${danger ? "hover:bg-[#c42b1c] hover:text-white" : "hover:bg-panel-3"}`}
    >
      {children}
    </button>
  );
}

export function TitleBar() {
  return (
    <header data-tauri-drag-region className="flex h-10 shrink-0 items-center pl-4">
      <div data-tauri-drag-region className="flex flex-1 items-center gap-2.5 text-[13px] font-medium text-ink-2">
        <Logo />
        Tally
      </div>
      {inTauri && (
        <div className="flex">
          <Button label="Minimize" onClick={() => win().minimize()}>
            <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5h10" stroke="currentColor" /></svg>
          </Button>
          <Button label="Maximize" onClick={() => win().toggleMaximize()}>
            <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" /></svg>
          </Button>
          <Button label="Close" onClick={() => win().close()} danger>
            <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" /></svg>
          </Button>
        </div>
      )}
    </header>
  );
}

/** Four tally strokes and the crossing fifth. */
export function Logo({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden>
      <g stroke="var(--claude)" strokeWidth="1.6" strokeLinecap="round">
        <path d="M3 3v10M6 3v10M9 3v10M12 3v10" />
        <path d="M1.5 11.5L14.5 4.5" stroke="var(--codex)" />
      </g>
    </svg>
  );
}
