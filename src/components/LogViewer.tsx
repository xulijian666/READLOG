import { Download } from "lucide-react";
import { invoke, saveDialog } from "../lib/runtime";
import { useQueryStore } from "../store/queryStore";
import type { LogEvent } from "../types/log";

const levelColors: Record<string, string> = {
  ERROR: "text-[#ef4444] bg-[#fee2e2]",
  WARN: "text-[#b45309] bg-[#fef3c7]",
  INFO: "text-[#047857] bg-[#d1fae5]",
  DEBUG: "text-[#4b5563] bg-[#e5e7eb]",
};

function renderHighlighted(event: LogEvent) {
  if (!event.highlighted || event.highlightRanges.length === 0) return event.firstLineContent;
  const range = event.highlightRanges[0];
  const source = event.rawText;
  const start = Math.max(0, Math.min(range.start, source.length));
  const end = Math.max(start, Math.min(range.end, source.length));
  const previewStart = Math.max(0, start - 70);
  const previewEnd = Math.min(source.length, end + 120);
  return (
    <>
      {previewStart > 0 ? "..." : ""}
      {source.slice(previewStart, start)}
      <mark className="rounded bg-[#fef08a] px-0.5">{source.slice(start, end)}</mark>
      {source.slice(end, previewEnd)}
      {previewEnd < source.length ? "..." : ""}
    </>
  );
}

export function LogViewer() {
  const { events } = useQueryStore();
  const visibleEvents = events.slice(0, 2000);

  const exportResults = async () => {
    const now = new Date();
    const stamp = [
      now.getFullYear(),
      String(now.getMonth() + 1).padStart(2, "0"),
      String(now.getDate()).padStart(2, "0"),
      String(now.getHours()).padStart(2, "0"),
      String(now.getMinutes()).padStart(2, "0"),
      String(now.getSeconds()).padStart(2, "0"),
    ].join("");
    const outputPath = await saveDialog({ defaultPath: `filtered_logs_${stamp}.txt` });
    if (!outputPath) return;
    await invoke("export_results", { events, outputPath });
  };

  return (
    <section className="flex min-h-0 flex-1 flex-col bg-[#f5f7fb]">
      <div className="flex items-center justify-between border-b border-[#d9e1ec] bg-white px-5 py-3">
        <h2 className="text-sm font-semibold text-[#243145]">日志</h2>
        <div className="flex items-center gap-3">
          <span className="text-sm text-[#607086]">
            共 {events.length.toLocaleString()} 条{events.length > visibleEvents.length ? `，显示前 ${visibleEvents.length.toLocaleString()} 条` : ""}
          </span>
          <button
            className="inline-flex items-center gap-2 rounded-md border border-[#cfd8e6] px-3 py-2 text-sm hover:bg-[#eef3f8] disabled:opacity-50"
            type="button"
            onClick={() => void exportResults()}
            disabled={!events.length}
          >
            <Download size={16} /> 下载筛选结果
          </button>
        </div>
      </div>
      <div className="log-scrollbar min-h-0 flex-1 overflow-auto p-4">
        <div className="min-w-[980px] overflow-hidden rounded-lg border border-[#d9e1ec] bg-white">
          {visibleEvents.length === 0 ? (
            <div className="px-5 py-16 text-center text-sm text-[#6b7280]">暂无日志</div>
          ) : (
            visibleEvents.map((event, index) => (
              <div
                key={`${event.id}-${index}`}
                className="grid items-start gap-3 border-b border-[#edf1f6] px-4 py-2.5 font-mono text-[13px] leading-6 last:border-b-0 hover:bg-[#f8fbff]"
                style={{ gridTemplateColumns: "96px 88px 1fr" }}
              >
                <span className="truncate rounded bg-[#e0ecff] px-2 py-0.5 text-center font-sans text-xs font-semibold text-[#1d4ed8]">
                  {event.serverName}
                </span>
                <span className={`rounded px-2 py-0.5 text-center text-xs font-semibold ${levelColors[event.level] ?? levelColors.DEBUG}`}>
                  {event.level}
                </span>
                <div className="min-w-0 break-words text-[#253044]">
                  <span className="mr-2 text-[#748197]">#{event.lineOffset}</span>
                  {renderHighlighted(event)}
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </section>
  );
}
