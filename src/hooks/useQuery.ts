import { invoke, isTauriRuntime } from "../lib/runtime";
import { useQueryStore } from "../store/queryStore";
import type { QueryRequest, ServerConfig } from "../types/query";

function toRfc3339(value: string) {
  if (!value) return null;
  return new Date(value).toISOString();
}

export function useQuery() {
  const query = useQueryStore();

  const execute = async (servers: ServerConfig[], batchSize: number) => {
    if (query.running && query.queryId) {
      await invoke("cancel_query", { queryId: query.queryId });
    }

    const queryId = crypto.randomUUID();
    const request: QueryRequest = {
      queryId,
      serverIds: servers.filter((server) => server.enabled).map((server) => server.id),
      filePath: query.filePath,
      startTime: toRfc3339(query.startTime),
      endTime: toRfc3339(query.endTime),
      keyword: query.keyword,
      level: query.level,
      batchSize,
    };

    query.resetResults();
    query.setQueryId(queryId);
    query.setRunning(true);
    if (!isTauriRuntime()) {
      const enabled = servers.filter((server) => server.enabled);
      const now = new Date().toISOString();
      query.setEvents(
        enabled.flatMap((server, index) => [
          {
            id: `${server.id}:1`,
            serverId: server.id,
            serverName: server.name,
            serverDisplayOrder: server.displayOrder,
            lineOffset: 1,
            timestamp: now,
            level: index % 2 === 0 ? "INFO" : "WARN",
            thread: "-exec-36",
            className: "com.readlog.Preview",
            firstLineContent: "26-04-30 13:00:02 INFO [-exec-36] com.readlog.Preview - browser preview sample event",
            rawText: `26-04-30 13:00:02 INFO [-exec-36] com.readlog.Preview - browser preview sample event\nkeyword=${query.keyword || "demo"}`,
            lineCount: 2,
            highlighted: Boolean(query.keyword),
            highlightRanges: query.keyword ? [{ start: 103, end: 103 + query.keyword.length }] : [],
          },
        ]),
      );
      query.setProgress({
        queryId,
        status: "completed",
        scannedBytes: 682847,
        scannedEvents: enabled.length,
        matchedEvents: enabled.length,
        serversCompleted: enabled.map((server) => server.id),
        serversPending: [],
      });
      query.setRunning(false);
      return;
    }
    try {
      await invoke("execute_query", { request });
    } catch (error) {
      query.setProgress({
        queryId,
        status: `error: ${String(error)}`,
        scannedBytes: 0,
        scannedEvents: 0,
        matchedEvents: 0,
        serversCompleted: [],
        serversPending: request.serverIds,
      });
      query.setRunning(false);
    }
  };

  const cancel = async () => {
    if (!query.queryId) return;
    await invoke("cancel_query", { queryId: query.queryId });
    query.setRunning(false);
  };

  return { execute, cancel };
}
