import { useCallback, useEffect, useState } from "react";
import { api, onEvent, type Overview, type ScanProgress } from "./api";

export type OverviewState = {
  overview: Overview | null;
  updatedAt: number;
  progress: ScanProgress | null;
  reload: () => void;
};

/** Overview data, reloaded whenever the backend says usage changed. */
export function useOverview(): OverviewState {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [updatedAt, setUpdatedAt] = useState(Date.now());
  const [progress, setProgress] = useState<ScanProgress | null>(null);

  const reload = useCallback(() => {
    api.overview().then((o) => {
      setOverview(o);
      setUpdatedAt(Date.now());
      if (!o.scanning) setProgress(null);
    });
  }, []);

  useEffect(() => {
    reload();
    const offs = [
      onEvent("usage-updated", reload),
      onEvent<ScanProgress>("scan-progress", setProgress),
    ];
    return () => offs.forEach((off) => off());
  }, [reload]);

  return { overview, updatedAt, progress, reload };
}
