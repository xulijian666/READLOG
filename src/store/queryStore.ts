import { create } from "zustand";
import type { LogEvent, LogLevel } from "../types/log";
import type { QueryProgressEvent } from "../types/query";

interface QueryState {
  queryId: string | null;
  events: LogEvent[];
  progress: QueryProgressEvent | null;
  running: boolean;
  filePath: string;
  startTime: string;
  endTime: string;
  keyword: string;
  level: LogLevel;
  setQueryId: (queryId: string | null) => void;
  setEvents: (events: LogEvent[]) => void;
  appendEvents: (events: LogEvent[]) => void;
  sortEvents: () => void;
  setProgress: (progress: QueryProgressEvent | null) => void;
  setRunning: (running: boolean) => void;
  setFilter: (patch: Partial<Pick<QueryState, "filePath" | "startTime" | "endTime" | "keyword" | "level">>) => void;
  resetResults: () => void;
}

export const useQueryStore = create<QueryState>((set) => ({
  queryId: null,
  events: [],
  progress: null,
  running: false,
  filePath: "app.log",
  startTime: "",
  endTime: "",
  keyword: "",
  level: "ALL",
  setQueryId: (queryId) => set({ queryId }),
  setEvents: (events) => set({ events }),
  appendEvents: (events) =>
    set((state) => {
      const byId = new Map<string, LogEvent>();
      for (const event of state.events) byId.set(event.id, event);
      for (const event of events) byId.set(event.id, event);
      return { events: Array.from(byId.values()) };
    }),
  sortEvents: () =>
    set((state) => ({
      events: [...state.events].sort((left, right) => {
        const timeDiff = new Date(left.timestamp).getTime() - new Date(right.timestamp).getTime();
        if (timeDiff !== 0) return timeDiff;
        if (left.serverDisplayOrder !== right.serverDisplayOrder) return left.serverDisplayOrder - right.serverDisplayOrder;
        return left.lineOffset - right.lineOffset;
      }),
    })),
  setProgress: (progress) => set({ progress }),
  setRunning: (running) => set({ running }),
  setFilter: (patch) => set(patch),
  resetResults: () => set({ events: [], progress: null }),
}));
