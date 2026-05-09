import { ChevronDown, ChevronRight, Search, XCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { invoke, listen } from "../lib/runtime";
import { useServerStore } from "../store/serverStore";
import type { LogSearchHit, LogSearchProgressEvent, LogSearchRequest, LogSearchResultEvent } from "../types/query";

type MatchMode = LogSearchRequest["matchMode"];

const matchModeLabels: Record<MatchMode, string> = {
  phrase: "完整匹配",
  all: "包含全部",
  any: "包含任一",
};

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function highlightLine(line: string, keyword: string, caseSensitive: boolean, matchMode: MatchMode) {
  if (!keyword.trim()) return line;
  const source = caseSensitive ? line : line.toLowerCase();
  const needles = matchMode === "phrase"
    ? [caseSensitive ? keyword : keyword.toLowerCase()]
    : keyword.split(/\s+/).filter(Boolean).map((part) => caseSensitive ? part : part.toLowerCase());
  const ranges = needles
    .flatMap((needle) => {
      const found: Array<{ start: number; end: number }> = [];
      let startAt = 0;
      while (needle && startAt < source.length) {
        const index = source.indexOf(needle, startAt);
        if (index < 0) break;
        found.push({ start: index, end: index + needle.length });
        startAt = index + needle.length;
      }
      return found;
    })
    .sort((left, right) => left.start - right.start)
    .reduce<Array<{ start: number; end: number }>>((merged, range) => {
      const previous = merged[merged.length - 1];
      if (!previous || range.start > previous.end) merged.push(range);
      else previous.end = Math.max(previous.end, range.end);
      return merged;
    }, []);
  if (!ranges.length) return line;
  return (
    <>
      {ranges.map((range, index) => {
        const previousEnd = index === 0 ? 0 : ranges[index - 1].end;
        return (
          <span key={`${range.start}-${range.end}`}>
            {line.slice(previousEnd, range.start)}
            <mark className="rounded bg-[#fef08a] px-0.5">{line.slice(range.start, range.end)}</mark>
          </span>
        );
      })}
      {line.slice(ranges[ranges.length - 1].end)}
    </>
  );
}

function getMatchModeHint(mode: MatchMode) {
  if (mode === "phrase") return "完整匹配：按输入内容整体查找。";
  if (mode === "all") return "包含全部：按空格拆词，同一行必须全部包含。";
  return "包含任一：按空格拆词，同一行包含任意一个即可。";
}

export function SearchPanel() {
  const config = useServerStore((state) => state.config);
  const [keyword, setKeyword] = useState("");
  const [matchMode, setMatchMode] = useState<MatchMode>("phrase");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [beforeLines, setBeforeLines] = useState("5");
  const [maxResults, setMaxResults] = useState("500");
  const [running, setRunning] = useState(false);
  const [error, setError] = useState("");
  const [results, setResults] = useState<LogSearchHit[]>([]);
  const [progress, setProgress] = useState<LogSearchProgressEvent | null>(null);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const queryIdRef = useRef<string | null>(null);

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    void listen<LogSearchResultEvent>("search-result", (event) => {
      const payload = event.payload;
      if (disposed || payload.queryId !== queryIdRef.current) return;
      if (payload.results.length) {
        setResults((current) => [...current, ...payload.results]);
      }
      if (payload.isLastBatch) {
        setRunning(false);
      }
    }).then((unlisten) => unlisteners.push(unlisten));

    void listen<LogSearchProgressEvent>("search-progress", (event) => {
      const payload = event.payload;
      if (disposed || payload.queryId !== queryIdRef.current) return;
      setProgress(payload);
      if (payload.status === "completed" || payload.status === "cancelled" || payload.status.startsWith("error:")) {
        setRunning(false);
        if (payload.status.startsWith("error:")) setError(payload.status);
      }
    }).then((unlisten) => unlisteners.push(unlisten));

    return () => {
      disposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  }, []);

  if (!config) return null;

  const enabledEntries = config.logEntries.filter((entry) => entry.enabled && entry.visible);

  const startSearch = async () => {
    setError("");
    const trimmed = keyword.trim();
    if (!trimmed) {
      setError("请输入关键词");
      return;
    }
    if (!enabledEntries.length) {
      setError("请先勾选至少一个日志 URL");
      return;
    }
    const contextCount = Math.max(0, Number.parseInt(beforeLines, 10) || 5);
    const max = Math.max(1, Number.parseInt(maxResults, 10) || 500);
    const queryId = crypto.randomUUID();
    queryIdRef.current = queryId;
    setResults([]);
    setProgress(null);
    setExpandedIds(new Set());
    setRunning(true);

    const request: LogSearchRequest = {
      queryId,
      logEntryIds: enabledEntries.map((entry) => entry.id),
      keyword: trimmed,
      matchMode,
      caseSensitive,
      beforeLines: contextCount,
      afterLines: contextCount,
      detailContextLines: 200,
      maxResults: max,
      batchSize: 50,
    };

    try {
      await invoke("search_log_files", { request });
    } catch (caught) {
      setError(String(caught));
      setRunning(false);
    }
  };

  const cancelSearch = async () => {
    if (!queryIdRef.current) return;
    await invoke("cancel_log_search", { queryId: queryIdRef.current });
    setRunning(false);
  };

  const toggleExpanded = (id: string) => {
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <section className="flex min-h-0 flex-1 flex-col bg-[#f5f7fb] px-5 py-5">
      <div className="rounded-lg border border-[#d9e1ec] bg-white p-5">
        <div className="grid items-end gap-4" style={{ gridTemplateColumns: "minmax(300px, 1fr) 140px 130px 130px auto auto" }}>
          <label className="block text-sm font-medium text-[#243145]">
            关键词
            <input
              className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
              value={keyword}
              onChange={(event) => setKeyword(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !running) void startSearch();
              }}
              placeholder="traceId / 订单号 / 接口 / Exception"
            />
          </label>
          <label className="block text-sm font-medium text-[#243145]">
            匹配方式
            <select
              className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
              value={matchMode}
              onChange={(event) => setMatchMode(event.target.value as MatchMode)}
            >
              {(Object.keys(matchModeLabels) as MatchMode[]).map((mode) => (
                <option key={mode} value={mode}>{matchModeLabels[mode]}</option>
              ))}
            </select>
          </label>
          <label className="block text-sm font-medium text-[#243145]">
            上下文行数
            <input
              className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
              value={beforeLines}
              onChange={(event) => setBeforeLines(event.target.value)}
            />
          </label>
          <label className="block text-sm font-medium text-[#243145]">
            最大结果
            <input
              className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
              value={maxResults}
              onChange={(event) => setMaxResults(event.target.value)}
            />
          </label>
          <label className="flex items-center gap-2 pb-2 text-sm text-[#4f6177]">
            <input
              type="checkbox"
              checked={caseSensitive}
              onChange={(event) => setCaseSensitive(event.target.checked)}
            />
            区分大小写
          </label>
          <div className="flex gap-2">
            <button
              className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50"
              type="button"
              onClick={() => void startSearch()}
              disabled={running}
            >
              <Search size={16} /> 查询
            </button>
            <button
              className="inline-flex items-center gap-2 rounded-md border border-[#cfd8e6] px-4 py-2.5 text-sm hover:bg-[#eef3f8] disabled:opacity-50"
              type="button"
              onClick={() => void cancelSearch()}
              disabled={!running}
            >
              <XCircle size={16} /> 取消
            </button>
          </div>
        </div>

        <div className="mt-4 flex flex-wrap items-center gap-3 text-xs text-[#69778c]">
          <span>查询范围：当前勾选的 {enabledEntries.length} 个单体日志 URL</span>
          <span>{getMatchModeHint(matchMode)}</span>
          {progress && (
            <>
              <span>扫描 {formatBytes(progress.scannedBytes)}</span>
              <span>{progress.scannedLines.toLocaleString()} 行</span>
              <span>命中 {results.length.toLocaleString()} 条</span>
              {running && progress.currentServer && <span>正在查：{progress.currentServer}</span>}
            </>
          )}
        </div>

        {error && <div className="mt-4 rounded-md bg-[#fff1f3] px-3 py-2 text-sm text-[#b42318]">{error}</div>}
      </div>

      <div className="mt-4 min-h-0 flex-1 overflow-auto rounded-lg border border-[#d9e1ec] bg-white">
        {results.length === 0 ? (
          <div className="px-5 py-16 text-center text-sm text-[#6b7280]">
            {running ? "正在查询，命中后会自动显示" : "暂无查询结果"}
          </div>
        ) : (
          <div className="divide-y divide-[#edf1f6]">
            {results.map((result, index) => {
              const expanded = expandedIds.has(result.id);
              const before = expanded ? result.detailBeforeLines : result.previewBeforeLines;
              const after = expanded ? result.detailAfterLines : result.previewAfterLines;
              return (
                <div key={`${result.id}-${index}`} className="px-4 py-3 hover:bg-[#f8fbff]">
                  <button
                    type="button"
                    className="mb-2 flex w-full items-center gap-2 text-left"
                    onClick={() => toggleExpanded(result.id)}
                  >
                    {expanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
                    <span className="rounded bg-[#e0ecff] px-2 py-0.5 text-xs font-semibold text-[#1d4ed8]">
                      {result.serverName}
                    </span>
                    <span className="text-xs text-[#69778c]">{result.fileName}</span>
                    <span className="text-xs text-[#69778c]">#{result.lineNumber}</span>
                    {expanded && <span className="text-xs text-[#047857]">完整上下文</span>}
                  </button>
                  <pre className="overflow-x-auto whitespace-pre-wrap rounded-md bg-[#0f172a] px-3 py-2 font-mono text-[12px] leading-5 text-[#dbeafe]">
                    {before.map((line, lineIndex) => (
                      <div key={`b-${lineIndex}`} className="text-[#94a3b8]">{line}</div>
                    ))}
                    <div className="bg-[#1e3a8a] text-white">{highlightLine(result.matchedLine, keyword.trim(), caseSensitive, matchMode)}</div>
                    {after.map((line, lineIndex) => (
                      <div key={`a-${lineIndex}`} className="text-[#cbd5e1]">{line}</div>
                    ))}
                  </pre>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}
