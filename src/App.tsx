import { useState } from "react";
import { TitleBar } from "./components/TitleBar";
import { Sidebar, type View } from "./components/Sidebar";
import { Overview } from "./components/Overview";
import { Agents } from "./components/Agents";
import { History } from "./components/History";
import { SettingsView } from "./components/Settings";
import { useOverview } from "./lib/useOverview";

export function App() {
  const [view, setView] = useState<View>("overview");
  const data = useOverview();

  return (
    <div className="flex h-full flex-col">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <Sidebar view={view} onChange={setView} />
        <main className="min-w-0 flex-1 overflow-y-auto px-8 pt-4 pb-8">
          {view === "overview" && <Overview data={data} onSettings={() => setView("settings")} />}
          {view === "agents" && <Agents data={data} />}
          {view === "history" && <History data={data} />}
          {view === "settings" && <SettingsView onChanged={data.reload} />}
        </main>
      </div>
    </div>
  );
}
