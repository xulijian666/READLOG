import { ArrowDown, ArrowUp, Check, Copy, List, Maximize2, Minimize2, Search, WrapText } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { LogSearchHit } from "../types/query";

type ReaderPreferences = { width: number; fontSize: number; wrap: boolean; listHidden: boolean };
const defaults: ReaderPreferences = { width: 280, fontSize: 13, wrap: true, listHidden: false };
function readPreferences(): ReaderPreferences {
  try {
    const saved = JSON.parse(localStorage.getItem("readlog.reader") || "{}");
    return {
      width: Number.isFinite(saved.width) ? Math.min(440, Math.max(200, saved.width)) : defaults.width,
      fontSize: [12, 13, 14, 16].includes(saved.fontSize) ? saved.fontSize : defaults.fontSize,
      wrap: typeof saved.wrap === "boolean" ? saved.wrap : defaults.wrap,
      listHidden: typeof saved.listHidden === "boolean" ? saved.listHidden : defaults.listHidden,
    };
  } catch { return defaults; }
}

interface Props {
  results: LogSearchHit[];
  running: boolean;
  focus: boolean;
  onToggleFocus: () => void;
  highlight: (line: string) => ReactNode;
}

export function SearchResults({ results, running, focus, onToggleFocus, highlight }: Props) {
  const [preferences, setPreferences] = useState(readPreferences);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const [copyStatus, setCopyStatus] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);
  const readerRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const hitRef = useRef<HTMLDivElement>(null);
  const scrollPositions = useRef(new Map<string, { top: number; left: number }>());
  const listScroll = useRef(0);
  const copyTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const copyVersion = useRef(0);
  const selectedIndex = Math.max(0, results.findIndex(result => result.id === selectedId));
  const selected = results[selectedIndex];
  const before = selected ? (expanded ? selected.detailBeforeLines : selected.previewBeforeLines) : [];
  const after = selected ? (expanded ? selected.detailAfterLines : selected.previewAfterLines) : [];
  const showList = !focus && !preferences.listHidden;

  useEffect(() => {
    try { localStorage.setItem("readlog.reader", JSON.stringify(preferences)); } catch { /* Storage may be unavailable. */ }
  }, [preferences]);

  useEffect(() => () => { clearTimeout(copyTimer.current); copyVersion.current++; }, []);

  useEffect(() => {
    copyVersion.current++;
    setCopyStatus("");
    clearTimeout(copyTimer.current);
    if (!results.length) { scrollPositions.current.clear(); listScroll.current = 0; }
  }, [selected?.id, results.length === 0]);

  useLayoutEffect(() => {
    if (!selected || !readerRef.current) return;
    const saved = scrollPositions.current.get(`${selected.id}:${expanded}`);
    if (saved) {
      readerRef.current.scrollTop = saved.top;
      readerRef.current.scrollLeft = saved.left;
    } else if (hitRef.current) {
      readerRef.current.scrollTop = Math.max(0, hitRef.current.offsetTop - readerRef.current.clientHeight / 3);
    }
  }, [selected?.id, expanded]);

  useLayoutEffect(() => {
    if (showList && listRef.current) listRef.current.scrollTop = listScroll.current;
  }, [showList]);

  useEffect(() => {
    const list = listRef.current;
    const active = list?.querySelector<HTMLElement>('[aria-current="true"]');
    if (!list || !active) return;
    if (active.offsetTop < list.scrollTop || active.offsetTop + active.offsetHeight > list.scrollTop + list.clientHeight) {
      list.scrollTop = active.offsetTop - list.clientHeight / 2;
    }
  }, [selected?.id]);

  const navigate = (direction: number) => {
    const next = results[selectedIndex + direction];
    if (next) setSelectedId(next.id);
  };

  const resize = (clientX: number) => {
    const bounds = containerRef.current?.getBoundingClientRect();
    if (!bounds) return;
    const width = Math.max(200, Math.min(440, bounds.width * .45, bounds.width - 360, clientX - bounds.left));
    setPreferences(current => ({ ...current, width }));
  };

  const copy = async () => {
    if (!selected) return;
    const version = ++copyVersion.current;
    try {
      await navigator.clipboard.writeText([...before, selected.matchedLine, ...after].join("\n"));
      if (version !== copyVersion.current) return;
      setCopyStatus("已复制");
    } catch {
      if (version !== copyVersion.current) return;
      setCopyStatus("复制失败，请选择日志文本复制");
    }
    clearTimeout(copyTimer.current);
    copyTimer.current = setTimeout(() => setCopyStatus(""), 3000);
  };

  const logLine = (text: string, number: number, matched = false) => (
    <div key={number} ref={matched ? hitRef : undefined} className={`reader-line ${matched ? "reader-match" : ""}`}>
      <span className="reader-number" aria-hidden="true">{number}</span>
      <span className="reader-text" style={{ whiteSpace: preferences.wrap ? "pre-wrap" : "pre" }}>{matched ? highlight(text) : text || " "}</span>
    </div>
  );

  return (
    <div ref={containerRef} className="search-results" data-testid="search-results">
      <div className="reader-toolbar">
        <div className="flex min-w-0 items-center gap-2">
          <button type="button" className="reader-tool" aria-label={preferences.listHidden ? "显示命中列表" : "隐藏命中列表"} disabled={focus} onClick={() => setPreferences(p => ({ ...p, listHidden: !p.listHidden }))}><List size={15} /></button>
          <span className="whitespace-nowrap text-sm font-semibold text-[#243145]">查询结果 <span className="ml-1 text-[#2563eb]">{results.length}</span></span>
          <span className="mx-1 h-4 border-l border-[#d9e1ec]" />
          <button type="button" className="reader-tool" aria-label="上一条命中" disabled={!selected || selectedIndex === 0} onClick={() => navigate(-1)}><ArrowUp size={15} /></button>
          <span className="whitespace-nowrap text-xs tabular-nums text-[#69778c]">{selected ? selectedIndex + 1 : 0} / {results.length}</span>
          <button type="button" className="reader-tool" aria-label="下一条命中" disabled={!selected || selectedIndex === results.length - 1} onClick={() => navigate(1)}><ArrowDown size={15} /></button>
        </div>
        <div className="flex items-center gap-1">
          <button type="button" className={`reader-tool ${preferences.wrap ? "reader-tool-active" : ""}`} aria-pressed={preferences.wrap} onClick={() => setPreferences(p => ({ ...p, wrap: !p.wrap }))}><WrapText size={14} /> 换行</button>
          <select aria-label="日志字号" className="reader-font" value={preferences.fontSize} onChange={event => setPreferences(p => ({ ...p, fontSize: Number(event.target.value) }))}>{[12, 13, 14, 16].map(size => <option key={size} value={size}>{size}px</option>)}</select>
          <button type="button" className="reader-tool" disabled={!selected} onClick={() => void copy()} title="复制当前显示的日志上下文">{copyStatus === "已复制" ? <Check size={14} /> : <Copy size={14} />} 复制</button>
          <button type="button" className="reader-tool" aria-pressed={focus} onClick={onToggleFocus}>{focus ? <Minimize2 size={14} /> : <Maximize2 size={14} />}{focus ? "退出专注" : "专注阅读"}</button>
        </div>
      </div>
      <div className="reader-split">
        {showList && <>
          <aside className="reader-list" style={{ width: preferences.width }} aria-label="命中列表">
            <div className="reader-list-heading">命中位置 <span>点击查看上下文</span></div>
            <div ref={listRef} className="min-h-0 flex-1 overflow-auto log-scrollbar" onScroll={event => { listScroll.current = event.currentTarget.scrollTop; }}>
              {results.map((result, index) => <button key={result.id} type="button" aria-current={selected?.id === result.id ? "true" : undefined} className={`reader-hit ${selected?.id === result.id ? "reader-hit-active" : ""}`} onClick={() => setSelectedId(result.id)}>
                <div className="mb-1 flex items-center justify-between gap-2"><span className="truncate font-semibold">{result.serverName}</span><span className="shrink-0 text-[11px] text-[#7c8aa0]">#{index + 1}</span></div>
                <div className="mb-1 flex gap-2 text-[11px] text-[#69778c]"><span className="truncate" title={result.fileName}>{result.fileName}</span><span className="shrink-0">行 {result.lineNumber}</span></div>
                <div className="line-clamp-2 break-all font-mono text-xs leading-5 text-[#607086]">{highlight(result.matchedLine)}</div>
              </button>)}
              {!results.length && <div className="p-5 text-xs leading-6 text-[#7c8aa0]">{running ? "正在扫描日志，命中后自动显示。" : "查询后在这里选择命中位置。"}</div>}
            </div>
          </aside>
          <div className="reader-divider" role="separator" tabIndex={0} aria-label="调整命中列表宽度" aria-orientation="vertical" aria-valuemin={200} aria-valuemax={440} aria-valuenow={Math.round(preferences.width)}
            onPointerDown={event => { event.currentTarget.setPointerCapture(event.pointerId); resize(event.clientX); }}
            onPointerMove={event => { if (event.currentTarget.hasPointerCapture(event.pointerId)) resize(event.clientX); }}
            onPointerUp={event => { if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); }}
            onKeyDown={event => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); const bounds = containerRef.current?.getBoundingClientRect(); if (bounds) resize(bounds.left + preferences.width + (event.key === "ArrowLeft" ? -20 : 20)); } }}><span /></div>
        </>}
        <div className="reader-detail">
          {selected ? <>
            <div className="reader-detail-heading"><div className="min-w-0"><span className="mr-2 font-semibold text-[#243145]">{selected.serverName}</span><span className="break-all text-[#69778c]">{selected.fileName}</span></div><button type="button" className="reader-tool shrink-0" aria-pressed={expanded} onClick={() => setExpanded(value => !value)}>{expanded ? "精简上下文" : "完整上下文"}</button></div>
            <div ref={readerRef} tabIndex={0} aria-label="日志详情" className="reader-code log-scrollbar" style={{ fontSize: preferences.fontSize }} onScroll={event => { scrollPositions.current.set(`${selected.id}:${expanded}`, { top: event.currentTarget.scrollTop, left: event.currentTarget.scrollLeft }); }}>
              <div className="relative min-w-full w-max py-3" style={{ width: preferences.wrap ? "100%" : undefined }}>
                {before.map((line, index) => logLine(line, selected.lineNumber - before.length + index))}
                {logLine(selected.matchedLine, selected.lineNumber, true)}
                {after.map((line, index) => logLine(line, selected.lineNumber + index + 1))}
              </div>
            </div>
            <div className="reader-footer"><span>命中行 {selected.lineNumber} · 前 {before.length} 行 / 后 {after.length} 行</span><span role="status">{copyStatus || (focus ? "按 Esc 退出专注阅读" : "可拖动分隔线调整阅读空间")}</span></div>
          </> : <div className="reader-empty"><Search size={34} strokeWidth={1.25} /><strong>{running ? "正在查询日志…" : "准备好阅读日志"}</strong><p>{running ? "查询结果会实时出现在左侧" : "输入关键词并查询，选择命中位置查看完整上下文"}</p></div>}
        </div>
      </div>
    </div>
  );
}
