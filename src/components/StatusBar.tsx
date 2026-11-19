import { Activity, AlertCircle, CheckCircle2, Loader2 } from "lucide-react";
import { useQueryStore } from "../store/queryStore";

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)}MB`;
}

export function StatusBar() {
  const { progress, running, events } = useQueryStore();
  const isError = progress?.status.startsWith("error");
  const statusText = progress
    ? `${progress.status} | 已扫描 ${formatBytes(progress.scannedBytes)} | 已解析 ${progress.scannedEvents} 条 | 命中 ${progress.matchedEvents} 条 | 完成 ${progress.serversCompleted.length} 台`
    : `就绪 | 当前 ${events.length} 条`;

  return (
    <section className={`flex min-h-[54px] items-center gap-3 border-b px-5 py-3 ${isError ? "border-[#fecdca] bg-[#fff1f3]" : "border-[#d9e1ec] bg-white"}`}>
      {running ? (
        <Loader2 className="animate-spin text-[#2563eb]" size={18} />
      ) : isError ? (
        <AlertCircle className="text-[#b42318]" size={18} />
      ) : progress?.status === "completed" ? (
        <CheckCircle2 className="text-[#10b981]" size={18} />
      ) : (
        <Activity className="text-[#6b7280]" size={18} />
      )}
      <span className={`text-sm ${isError ? "text-[#b42318]" : "text-[#334155]"}`}>{statusText}</span>
    </section>
  );
}
