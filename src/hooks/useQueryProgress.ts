import { useEffect } from "react";
import { listen } from "../lib/runtime";
import type { QueryProgressEvent, QueryResultEvent } from "../types/query";
import { useQueryStore } from "../store/queryStore";

export function useQueryProgress() {
  useEffect(() => {
    let disposed = false;
    const unsubs: Array<() => void> = [];

    void listen<QueryResultEvent>("query-result", (event) => {
      if (disposed) return;
      const state = useQueryStore.getState();
      if (event.payload.queryId !== state.queryId) return;
      if (event.payload.events.length > 0) {
        state.appendEvents(event.payload.events);
      }
      if (event.payload.isLastBatch) {
        state.sortEvents();
        state.setRunning(false);
      }
    }).then((unsub) => unsubs.push(unsub));

    void listen<QueryProgressEvent>("query-progress", (event) => {
      if (disposed) return;
      const state = useQueryStore.getState();
      if (event.payload.queryId !== state.queryId) return;
      state.setProgress(event.payload);
      if (event.payload.status === "completed" || event.payload.status === "cancelled" || event.payload.status.startsWith("error")) {
        state.setRunning(false);
      }
    }).then((unsub) => unsubs.push(unsub));

    return () => {
      disposed = true;
      unsubs.forEach((unsub) => unsub());
    };
  }, []);
}
