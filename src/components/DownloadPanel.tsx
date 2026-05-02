import { Archive, Download, FileText, FolderOpen, Scissors } from "lucide-react";
import { useState } from "react";
import { invoke } from "../lib/runtime";
import { useServerStore } from "../store/serverStore";
import type { DownloadSummary } from "../types/query";

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

type DownloadMode = "realtime" | "archive" | "tail";

interface ArchiveParams {
  month: string;
  day: string;
  hourStart: string;
  hourEnd: string;
}

export function DownloadPanel() {
  const config = useServerStore((state) => state.config);
  const [mode, setMode] = useState<DownloadMode>("realtime");
  const [downloading, setDownloading] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [outputPath, setOutputPath] = useState("");
  const [archiveParams, setArchiveParams] = useState<ArchiveParams>({
    month: getCurrentMonth(),
    day: getCurrentDay(),
    hourStart: "",
    hourEnd: "",
  });
  const [tailLineCount, setTailLineCount] = useState("500");

  if (!config) return null;

  const enabledServers = config.servers.filter((server) => server.enabled);
  const logType = config.settings.logType || "app";

  function getCurrentMonth(): string {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  }

  function getCurrentDay(): string {
    const now = new Date();
    return String(now.getDate()).padStart(2, "0");
  }

  function getAvailableMonths(): string[] {
    const now = new Date();
    const current = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
    const prevMonth = now.getMonth() === 0 ? 12 : now.getMonth();
    const prevYear = now.getMonth() === 0 ? now.getFullYear() - 1 : now.getFullYear();
    const previous = `${prevYear}-${String(prevMonth).padStart(2, "0")}`;
    return [current, previous];
  }

  const downloadRealtime = async () => {
    setError("");
    setDownloading(true);
    setOutputPath("");
    setMessage(`正在下载 ${enabledServers.length} 个服务器的 ${logType}.log...`);
    try {
      const summary = await invoke<DownloadSummary>("download_realtime_logs", {
        serverIds: enabledServers.map((server) => server.id),
        logType,
        outputPath: "",
      });
      setOutputPath(summary.outputPath);
      setMessage(`已下载 ${summary.serverCount} 个日志，写入 ${formatBytes(summary.bytesWritten)}：${summary.outputPath}`);
    } catch (caught) {
      setError(String(caught));
      setMessage("");
    } finally {
      setDownloading(false);
    }
  };

  const downloadArchive = async () => {
    setError("");
    const { month, day, hourStart, hourEnd } = archiveParams;
    if (!hourStart || !hourEnd) {
      setError("请填写小时范围");
      return;
    }
    const start = parseInt(hourStart, 10);
    const end = parseInt(hourEnd, 10);
    if (start < 0 || start > 23 || end < 0 || end > 23) {
      setError("小时范围必须在 0-23 之间");
      return;
    }
    if (start > end) {
      setError("起始小时不能大于结束小时");
      return;
    }
    setDownloading(true);
    setOutputPath("");
    setMessage(`正在下载 ${enabledServers.length} 个服务器的归档日志 (${month}/${day} ${hourStart}-${hourEnd}时)...`);
    try {
      const summary = await invoke<DownloadSummary>("download_archive_logs", {
        serverIds: enabledServers.map((server) => server.id),
        logType,
        month,
        day,
        hourStart,
        hourEnd,
        outputPath: "",
      });
      if (summary.serverCount === 0) {
        setError("指定时间范围内没有找到归档日志文件");
        setMessage("");
      } else {
        setOutputPath(summary.outputPath);
        setMessage(`已下载 ${summary.serverCount} 个归档，写入 ${formatBytes(summary.bytesWritten)}：${summary.outputPath}`);
      }
    } catch (caught) {
      setError(String(caught));
      setMessage("");
    } finally {
      setDownloading(false);
    }
  };

  const downloadTail = async () => {
    setError("");
    const count = parseInt(tailLineCount, 10);
    if (!count || count <= 0) {
      setError("请输入有效的行数");
      return;
    }
    setDownloading(true);
    setOutputPath("");
    setMessage(`正在截取 ${enabledServers.length} 个服务器的 ${logType}.log 最后 ${count} 行...`);
    try {
      const summary = await invoke<DownloadSummary>("download_tail_logs", {
        serverIds: enabledServers.map((server) => server.id),
        logType,
        lineCount: tailLineCount,
        outputPath: "",
      });
      setOutputPath(summary.outputPath);
      setMessage(`已截取 ${summary.serverCount} 个日志各 ${count} 行，写入 ${formatBytes(summary.bytesWritten)}：${summary.outputPath}`);
    } catch (caught) {
      setError(String(caught));
      setMessage("");
    } finally {
      setDownloading(false);
    }
  };

  const handleOpenFile = async () => {
    if (!outputPath) return;
    try {
      await invoke("open_file", { path: outputPath });
    } catch (caught) {
      setError(String(caught));
    }
  };

  const handleOpenFolder = async () => {
    if (!outputPath) return;
    try {
      await invoke("open_folder", { path: outputPath });
    } catch (caught) {
      setError(String(caught));
    }
  };

  return (
    <section className="flex flex-1 flex-col bg-[#f5f7fb] px-5 py-5">
      <div className="rounded-lg border border-[#d9e1ec] bg-white p-5">
        <div className="mb-4 flex items-center gap-4 border-b border-[#e3e8f0] pb-4">
          <button
            type="button"
            className={`inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md ${
              mode === "realtime"
                ? "bg-[#2563eb] text-white"
                : "border border-[#cfd8e6] text-[#69778c] hover:bg-[#eef3f8]"
            }`}
            onClick={() => setMode("realtime")}
          >
            <Download size={16} /> 实时日志
          </button>
          <button
            type="button"
            className={`inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md ${
              mode === "archive"
                ? "bg-[#2563eb] text-white"
                : "border border-[#cfd8e6] text-[#69778c] hover:bg-[#eef3f8]"
            }`}
            onClick={() => setMode("archive")}
          >
            <Archive size={16} /> 归档日志
          </button>
          <button
            type="button"
            className={`inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md ${
              mode === "tail"
                ? "bg-[#2563eb] text-white"
                : "border border-[#cfd8e6] text-[#69778c] hover:bg-[#eef3f8]"
            }`}
            onClick={() => setMode("tail")}
          >
            <Scissors size={16} /> 截取日志
          </button>
        </div>

        {mode === "realtime" && (
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-base font-semibold text-[#243145]">下载实时日志</h3>
              <p className="mt-1 text-sm text-[#69778c]">
                下载勾选服务器的 {logType}.log 完整实时日志文件，按顺序合并。
              </p>
            </div>
            <button
              className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-5 py-2.5 font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50"
              type="button"
              onClick={() => void downloadRealtime()}
              disabled={downloading || enabledServers.length === 0}
            >
              <Download size={18} /> {downloading ? "下载中" : `下载 (${enabledServers.length})`}
            </button>
          </div>
        )}

        {mode === "archive" && (
          <div>
            <h3 className="text-base font-semibold text-[#243145]">下载归档日志</h3>
            <p className="mt-1 text-sm text-[#69778c]">
              按时间范围下载勾选服务器的 {logType} 归档日志（.gz 文件），解压后合并。
            </p>
            <div className="mt-4 flex flex-wrap items-end gap-4">
              <label className="block text-sm font-medium">
                月份
                <select
                  className="mt-1 rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
                  value={archiveParams.month}
                  onChange={(e) => setArchiveParams({ ...archiveParams, month: e.target.value })}
                >
                  {getAvailableMonths().map((m) => (
                    <option key={m} value={m}>{m}</option>
                  ))}
                </select>
              </label>
              <label className="block text-sm font-medium">
                日期
                <input
                  className="mt-1 w-16 rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
                  type="text"
                  placeholder="01"
                  value={archiveParams.day}
                  onChange={(e) => setArchiveParams({ ...archiveParams, day: e.target.value })}
                />
              </label>
              <label className="block text-sm font-medium">
                小时范围
                <div className="mt-1 flex items-center gap-2">
                  <input
                    className="w-12 rounded-md border border-[#cfd8e6] bg-white px-2 py-2 outline-none focus:border-[#2563eb]"
                    type="text"
                    placeholder="00"
                    value={archiveParams.hourStart}
                    onChange={(e) => setArchiveParams({ ...archiveParams, hourStart: e.target.value })}
                  />
                  <span className="text-[#69778c]">-</span>
                  <input
                    className="w-12 rounded-md border border-[#cfd8e6] bg-white px-2 py-2 outline-none focus:border-[#2563eb]"
                    type="text"
                    placeholder="23"
                    value={archiveParams.hourEnd}
                    onChange={(e) => setArchiveParams({ ...archiveParams, hourEnd: e.target.value })}
                  />
                </div>
              </label>
              <button
                className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-5 py-2.5 font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50"
                type="button"
                onClick={() => void downloadArchive()}
                disabled={downloading || enabledServers.length === 0}
              >
                <Archive size={18} /> {downloading ? "下载中" : `下载归档 (${enabledServers.length})`}
              </button>
            </div>
          </div>
        )}

        {mode === "tail" && (
          <div>
            <h3 className="text-base font-semibold text-[#243145]">截取日志尾部</h3>
            <p className="mt-1 text-sm text-[#69778c]">
              获取勾选服务器的 {logType}.log 最后 N 行，按顺序合并。
            </p>
            <div className="mt-4 flex items-end gap-4">
              <label className="block text-sm font-medium">
                截取行数
                <input
                  className="mt-1 w-24 rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
                  type="text"
                  placeholder="500"
                  value={tailLineCount}
                  onChange={(e) => setTailLineCount(e.target.value)}
                />
              </label>
              <button
                className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-5 py-2.5 font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50"
                type="button"
                onClick={() => void downloadTail()}
                disabled={downloading || enabledServers.length === 0}
              >
                <Scissors size={18} /> {downloading ? "截取中" : `截取 (${enabledServers.length})`}
              </button>
            </div>
          </div>
        )}

        {message && (
          <div className="mt-4 flex items-center gap-3 rounded-md bg-[#ecfdf3] px-3 py-2">
            <span className="text-sm text-[#047857]">{message}</span>
            {outputPath && (
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => void handleOpenFile()}
                  className="inline-flex items-center gap-1 rounded border border-[#047857] px-2 py-1 text-xs text-[#047857] hover:bg-[#047857] hover:text-white"
                >
                  <FileText size={14} /> 打开文件
                </button>
                <button
                  type="button"
                  onClick={() => void handleOpenFolder()}
                  className="inline-flex items-center gap-1 rounded border border-[#047857] px-2 py-1 text-xs text-[#047857] hover:bg-[#047857] hover:text-white"
                >
                  <FolderOpen size={14} /> 打开文件夹
                </button>
              </div>
            )}
          </div>
        )}
        {error && <div className="mt-4 rounded-md bg-[#fff1f3] px-3 py-2 text-sm text-[#b42318]">{error}</div>}
      </div>
    </section>
  );
}