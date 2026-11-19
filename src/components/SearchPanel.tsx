import { Archive, ChevronDown, ChevronRight, FolderOpen, RefreshCw, Search, Settings2, SlidersHorizontal, X, XCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { invoke, listen } from "../lib/runtime";
import { useServerStore } from "../store/serverStore";
import { SearchResults } from "./SearchResults";
import type { DirEntry, LogSearchHit, LogSearchProgressEvent, LogSearchRequest, LogSearchResultEvent } from "../types/query";

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

function CustomCheckbox({ checked }: { checked: boolean }) {
  return (
    <span
      className={`flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded border ${
        checked ? "border-[#2563eb] bg-[#2563eb]" : "border-[#cfd8e6] bg-white"
      }`}
      aria-hidden="true"
    >
      {checked && (
        <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
          <path d="M1 4L3.5 6.5L9 1" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      )}
    </span>
  );
}

export function SearchPanel({ onOpenScope }: { onOpenScope: () => void }) {
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
  const [advanced, setAdvanced] = useState(() => {
    try { return localStorage.getItem("readlog.search.advanced") === "true"; } catch { return false; }
  });
  const [focus, setFocus] = useState(false);
  const [appliedQuery, setAppliedQuery] = useState({ keyword: "", caseSensitive: false, matchMode: "phrase" as MatchMode });
  useEffect(() => {
    try { localStorage.setItem("readlog.search.advanced", String(advanced)); } catch { /* Optional preference. */ }
  }, [advanced]);
  useEffect(() => {
    if (!focus) return;
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") setFocus(false); };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, [focus]);
  const queryIdRef = useRef<string | null>(null);

  // Archive search state
  const [archiveMode, setArchiveMode] = useState(false);
  const [archiveEntryId, setArchiveEntryId] = useState("");
  const [archiveFiles, setArchiveFiles] = useState<DirEntry[]>([]);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [showModal, setShowModal] = useState(false);
  const [breadcrumbs, setBreadcrumbs] = useState<Array<{ name: string; url: string }>>([]);

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

  const openArchiveModal = async () => {
    setError("");
    if (!archiveEntryId) {
      setError("请先选择一个日志路径");
      return;
    }
    setLoadingFiles(true);
    setArchiveFiles([]);
    setSelectedFiles(new Set());
    setBreadcrumbs([]);
    try {
      const files = await invoke<DirEntry[]>("list_archive_files", {
        logEntryId: archiveEntryId,
      });
      if (files.length === 0) {
        setError("该目录下没有找到归档日志文件");
      } else {
        setArchiveFiles(files);
        setSelectedFiles(new Set(files.filter((f) => !f.isDir).map((f) => f.url)));
        setShowModal(true);
      }
    } catch (caught) {
      setError(String(caught));
    } finally {
      setLoadingFiles(false);
    }
  };

  const navigateToFolder = async (folder: DirEntry) => {
    setLoadingFiles(true);
    try {
      const files = await invoke<DirEntry[]>("list_archive_subdir", {
        dirUrl: folder.url,
      });
      setBreadcrumbs((prev) => [...prev, { name: folder.name, url: folder.url }]);
      setArchiveFiles(files);
      setSelectedFiles(new Set(files.filter((f) => !f.isDir).map((f) => f.url)));
    } catch (caught) {
      setError(String(caught));
    } finally {
      setLoadingFiles(false);
    }
  };

  const navigateToBreadcrumb = async (index: number) => {
    setLoadingFiles(true);
    try {
      const target = index < 0
        ? await invoke<DirEntry[]>("list_archive_files", { logEntryId: archiveEntryId })
        : await invoke<DirEntry[]>("list_archive_subdir", { dirUrl: breadcrumbs[index].url });
      setBreadcrumbs((prev) => prev.slice(0, index + 1));
      setArchiveFiles(target);
      setSelectedFiles(new Set(target.filter((f) => !f.isDir).map((f) => f.url)));
    } catch (caught) {
      setError(String(caught));
    } finally {
      setLoadingFiles(false);
    }
  };

  const toggleFile = (url: string) => {
    setSelectedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(url)) next.delete(url);
      else next.add(url);
      return next;
    });
  };

  const toggleAllFiles = () => {
    const fileEntries = archiveFiles.filter((f) => !f.isDir);
    if (selectedFiles.size === fileEntries.length) {
      setSelectedFiles(new Set());
    } else {
      setSelectedFiles(new Set(fileEntries.map((f) => f.url)));
    }
  };

  const startSearch = async () => {
    setError("");
    const trimmed = keyword.trim();
    if (!trimmed) {
      setError("请输入关键词");
      return;
    }

    if (archiveMode) {
      if (selectedFiles.size === 0) {
        setError("请先加载并选择归档文件");
        return;
      }
    } else {
      if (!enabledEntries.length) {
        setError("请先勾选至少一个日志 URL");
        return;
      }
    }

    const parsedContext = Number.parseInt(beforeLines, 10);
    const contextCount = Math.max(0, Number.isFinite(parsedContext) ? parsedContext : 5);
    const max = Math.max(1, Number.parseInt(maxResults, 10) || 500);
    const queryId = crypto.randomUUID();
    queryIdRef.current = queryId;
    setResults([]);
    setProgress(null);
    setAppliedQuery({ keyword: trimmed, caseSensitive, matchMode });
    setRunning(true);

    try {
      if (archiveMode) {
        await invoke("search_archive_files", {
          request: {
            queryId,
            fileUrls: Array.from(selectedFiles),
            keyword: trimmed,
            matchMode,
            caseSensitive,
            beforeLines: contextCount,
            afterLines: contextCount,
            detailContextLines: 200,
            maxResults: max,
            batchSize: 50,
          },
        });
      } else {
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
        await invoke("search_log_files", { request });
      }
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

  const scopeLabel = archiveMode
    ? `归档文件 ${selectedFiles.size} 个`
    : `当前勾选的 ${enabledEntries.length} 个单体日志 URL`;

  const fileCount = archiveFiles.filter((f) => !f.isDir).length;
  const allSelected = selectedFiles.size === fileCount && fileCount > 0;
  const someSelected = selectedFiles.size > 0 && selectedFiles.size < fileCount;

  return (
    <section className={focus ? "fixed inset-0 z-40 flex min-h-0 flex-col bg-[#f5f7fb] p-3" : "flex min-h-0 flex-1 flex-col gap-3 overflow-hidden bg-[#f5f7fb] px-4 py-3"}>
      {!focus && <div className="shrink-0 rounded-lg border border-[#d9e1ec] bg-white px-3 py-2.5">
        <div className="flex items-center gap-2">
          <div className="relative min-w-[200px] flex-1">
            <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-[#94a3b8]" />
            <input aria-label="关键词" className="w-full rounded-md border border-[#cfd8e6] bg-white py-2 pl-9 pr-3 text-sm outline-none focus:border-[#2563eb]" value={keyword} onChange={event => setKeyword(event.target.value)} onKeyDown={event => { if (event.key === "Enter" && !running) void startSearch(); }} placeholder="输入 traceId / 订单号 / 接口 / Exception" />
          </div>
          <select aria-label="匹配方式" title={getMatchModeHint(matchMode)} className="rounded-md border border-[#cfd8e6] bg-white px-2 py-2 text-sm outline-none focus:border-[#2563eb]" value={matchMode} onChange={event => setMatchMode(event.target.value as MatchMode)}>
            {(Object.keys(matchModeLabels) as MatchMode[]).map(mode => <option key={mode} value={mode}>{matchModeLabels[mode]}</option>)}
          </select>
          <button type="button" className="reader-tool border border-[#d9e1ec] py-2" onClick={onOpenScope}><SlidersHorizontal size={14} /> 查询范围 · {enabledEntries.length}</button>
          <button type="button" className="reader-tool py-2" aria-expanded={advanced} onClick={() => setAdvanced(value => !value)}><Settings2 size={14} /> 高级选项</button>
          <button type="button" className="inline-flex shrink-0 items-center gap-1.5 rounded-md bg-[#2563eb] px-4 py-2 text-sm font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50" disabled={running} onClick={() => void startSearch()}><Search size={15} /> 查询</button>
          {running && <button type="button" className="reader-tool border border-[#d9e1ec] py-2" onClick={() => void cancelSearch()}><XCircle size={14} /> 取消</button>}
        </div>
        <div className="mt-2 flex items-center gap-3 text-xs text-[#69778c]">
          <label className="inline-flex shrink-0 cursor-pointer items-center gap-1.5"><input type="checkbox" className="accent-[#2563eb]" checked={archiveMode} disabled={running} onChange={event => { setArchiveMode(event.target.checked); setError(""); setResults([]); setProgress(null); }} /><Archive size={13} /> 归档查询</label>
          {archiveMode ? <>
            <select aria-label="归档日志路径" className="max-w-[250px] rounded border border-[#d9e1ec] px-2 py-1 outline-none focus:border-[#2563eb]" disabled={running} value={archiveEntryId} onChange={event => { setArchiveEntryId(event.target.value); setSelectedFiles(new Set()); setArchiveFiles([]); setError(""); }}>
              <option value="">选择日志路径</option>{enabledEntries.map(entry => <option key={entry.id} value={entry.id}>{entry.name}</option>)}
            </select>
            <button type="button" className="reader-tool py-0.5" onClick={() => void openArchiveModal()} disabled={running || loadingFiles || !archiveEntryId}><RefreshCw size={12} className={loadingFiles ? "animate-spin" : ""} />{loadingFiles ? "加载中…" : "选择文件"}</button>
            <span>已选 {selectedFiles.size} 个归档文件</span>
          </> : <span className="truncate">查询范围：{scopeLabel}</span>}
        </div>
        {advanced && <div className="mt-2 flex items-center gap-5 border-t border-[#edf1f6] pt-2 text-xs text-[#607086]">
          <label className="flex items-center gap-2">上下文行数 <input aria-label="上下文行数" type="number" min="0" className="w-20 rounded border border-[#cfd8e6] px-2 py-1" value={beforeLines} onChange={event => setBeforeLines(event.target.value)} /></label>
          <label className="flex items-center gap-2">最大结果 <input aria-label="最大结果" type="number" min="1" className="w-20 rounded border border-[#cfd8e6] px-2 py-1" value={maxResults} onChange={event => setMaxResults(event.target.value)} /></label>
          <label className="flex cursor-pointer items-center gap-2"><input type="checkbox" checked={caseSensitive} onChange={event => setCaseSensitive(event.target.checked)} />区分大小写</label>
          <span>{getMatchModeHint(matchMode)}</span>
        </div>}
      </div>}
      <div className={`flex shrink-0 items-center justify-between gap-3 text-xs text-[#69778c] ${focus ? "mb-2 px-1" : "px-1"}`} role="status">
        <div className="flex min-w-0 items-center gap-3"><span className={`inline-flex shrink-0 items-center gap-1.5 ${running ? "text-[#2563eb]" : ""}`}><span className={`h-1.5 w-1.5 rounded-full ${running ? "animate-pulse bg-[#2563eb]" : "bg-[#94a3b8]"}`} />{running ? "正在查询" : progress?.status === "cancelled" ? "已取消" : progress?.status === "completed" ? "查询完成" : "就绪"}</span>{progress && <><span className="shrink-0">扫描 {formatBytes(progress.scannedBytes)} · {progress.scannedLines.toLocaleString()} 行</span><span className="truncate">{running ? progress.currentServer : ""}</span></>}</div>
        <div className="flex shrink-0 items-center gap-3"><span>命中 {results.length.toLocaleString()} 条</span>{focus && running && <button type="button" className="reader-tool" onClick={() => void cancelSearch()}>取消查询</button>}</div>
      </div>
      {error && <div role="alert" className="shrink-0 rounded-md bg-[#fff1f3] px-3 py-2 text-xs text-[#b42318]">{error}</div>}
      <SearchResults results={results} running={running} focus={focus} onToggleFocus={() => setFocus(value => !value)} highlight={line => highlightLine(line, appliedQuery.keyword, appliedQuery.caseSensitive, appliedQuery.matchMode)} />

      {/* Archive file picker modal */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" onClick={() => setShowModal(false)}>
          <div className="flex w-[560px] max-h-[80vh] flex-col rounded-lg bg-white shadow-xl" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center justify-between border-b border-[#e3e8f0] px-5 py-3">
              <h3 className="text-sm font-semibold text-[#243145]">
                选择归档文件 — {enabledEntries.find((e) => e.id === archiveEntryId)?.name}
              </h3>
              <button className="rounded p-1 text-[#9ca3af] hover:bg-[#f3f4f6] hover:text-[#243145]" onClick={() => setShowModal(false)}>
                <X size={18} />
              </button>
            </div>

            {/* Breadcrumbs */}
            <div className="flex items-center gap-1 border-b border-[#e8ecf2] px-5 py-2 text-xs text-[#69778c]">
              <button
                className="rounded px-1 py-0.5 text-[#2563eb] hover:bg-[#eef3f8]"
                onClick={() => void navigateToBreadcrumb(-1)}
              >
                根目录
              </button>
              {breadcrumbs.map((crumb, i) => (
                <span key={crumb.url} className="flex items-center gap-1">
                  <ChevronRight size={12} />
                  <button
                    className="rounded px-1 py-0.5 text-[#2563eb] hover:bg-[#eef3f8]"
                    onClick={() => void navigateToBreadcrumb(i)}
                  >
                    {crumb.name}
                  </button>
                </span>
              ))}
            </div>

            <div className="flex items-center justify-between border-b border-[#e8ecf2] px-5 py-2">
              <label className="-m-2 inline-flex cursor-pointer items-center gap-2 p-2 text-sm text-[#243145]">
                <input className="sr-only" type="checkbox" checked={allSelected} onChange={toggleAllFiles} />
                <span
                  className={`flex h-4 w-4 items-center justify-center rounded border ${
                    allSelected ? "border-[#2563eb] bg-[#2563eb]" : someSelected ? "border-[#2563eb] bg-[#2563eb]" : "border-[#cfd8e6] bg-white"
                  }`}
                  aria-hidden="true"
                >
                  {allSelected ? (
                    <svg width="10" height="8" viewBox="0 0 10 8" fill="none"><path d="M1 4L3.5 6.5L9 1" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>
                  ) : someSelected ? (
                    <svg width="10" height="2" viewBox="0 0 10 2" fill="none"><path d="M1 1H9" stroke="white" strokeWidth="1.5" strokeLinecap="round" /></svg>
                  ) : null}
                </span>
                全选
              </label>
              <span className="text-xs text-[#69778c]">已选 {selectedFiles.size} / {fileCount}</span>
            </div>

            <div className="flex-1 overflow-y-auto px-1">
              {loadingFiles ? (
                <div className="flex items-center justify-center py-8 text-sm text-[#69778c]">加载中...</div>
              ) : archiveFiles.length === 0 ? (
                <div className="flex items-center justify-center py-8 text-sm text-[#69778c]">该目录为空</div>
              ) : (
                archiveFiles.map((file) =>
                  file.isDir ? (
                    <button
                      key={file.url}
                      type="button"
                      className="flex w-full cursor-pointer items-center gap-2.5 rounded px-4 py-2 text-sm hover:bg-[#f0f4ff]"
                      onClick={() => void navigateToFolder(file)}
                    >
                      <FolderOpen size={16} className="shrink-0 text-[#f59e0b]" />
                      <span className="flex-1 truncate text-left text-[#243145]">{file.name}</span>
                      <ChevronRight size={14} className="shrink-0 text-[#9ca3af]" />
                    </button>
                  ) : (
                    <label key={file.url} className="flex cursor-pointer items-center gap-2.5 rounded px-4 py-2 text-sm hover:bg-[#f0f4ff]">
                      <input className="sr-only" type="checkbox" checked={selectedFiles.has(file.url)} onChange={() => toggleFile(file.url)} />
                      <CustomCheckbox checked={selectedFiles.has(file.url)} />
                      <span className="flex-1 truncate text-[#243145]">{file.name}</span>
                      {file.name.endsWith(".gz") && (
                        <span className="shrink-0 rounded bg-[#dbeafe] px-1.5 py-0.5 text-[10px] font-medium text-[#2563eb]">gz</span>
                      )}
                    </label>
                  )
                )
              )}
            </div>

            <div className="flex items-center justify-end gap-3 border-t border-[#e3e8f0] px-5 py-3">
              <button
                className="rounded-md border border-[#cfd8e6] bg-white px-4 py-2 text-sm text-[#69778c] hover:bg-[#f3f4f6]"
                onClick={() => setShowModal(false)}
              >
                取消
              </button>
              <button
                className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-5 py-2 text-sm font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50"
                onClick={() => setShowModal(false)}
                disabled={selectedFiles.size === 0}
              >
                确定 ({selectedFiles.size})
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
