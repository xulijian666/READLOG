import { Archive, Bot, ChevronRight, Copy, Download, FileText, FolderInput, FolderOpen, RefreshCw, Scissors, X } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke, pickDirectory } from "../lib/runtime";
import { useServerStore } from "../store/serverStore";
import type { DirEntry, DownloadSummary } from "../types/query";

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

type DownloadMode = "realtime" | "archive" | "tail";

function CustomCheckbox({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  return (
    <span
      className={`flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded border ${
        checked ? "border-[#2563eb] bg-[#2563eb]" : "border-[#cfd8e6] bg-white"
      }`}
      onClick={onChange}
    >
      {checked && (
        <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
          <path d="M1 4L3.5 6.5L9 1" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      )}
    </span>
  );
}

export function DownloadPanel() {
  const config = useServerStore((state) => state.config);
  const [mode, setMode] = useState<DownloadMode>("realtime");
  const [downloading, setDownloading] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [outputPath, setOutputPath] = useState("");
  // Archive state
  const [archiveEntryId, setArchiveEntryId] = useState("");
  const [archiveFiles, setArchiveFiles] = useState<DirEntry[]>([]);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [showModal, setShowModal] = useState(false);
  const [breadcrumbs, setBreadcrumbs] = useState<Array<{ name: string; url: string }>>([]);
  const [tailLineCount, setTailLineCount] = useState("500");
  const [agentInstalled, setAgentInstalled] = useState(false);

  useEffect(() => {
    invoke<{ installed: boolean }>("check_agent_status")
      .then((result) => setAgentInstalled(result.installed))
      .catch(() => setAgentInstalled(false));
  }, []);

  if (!config) return null;

  const enabledEntries = config.logEntries.filter((entry) => entry.enabled && entry.visible);

  const downloadRealtime = async () => {
    setError("");
    setDownloading(true);
    setOutputPath("");
    setMessage(`正在下载 ${enabledEntries.length} 条日志...`);
    try {
      const summary = await invoke<DownloadSummary>("download_realtime_logs", {
        logEntryIds: enabledEntries.map((e) => e.id),
        outputPath: "",
      });
      setOutputPath(summary.outputPath);
      setMessage(`已下载 ${summary.serverCount} 条日志，写入 ${formatBytes(summary.bytesWritten)}`);
    } catch (caught) {
      setError(String(caught));
      setMessage("");
    } finally {
      setDownloading(false);
    }
  };

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

  const downloadSelectedArchive = async () => {
    setError("");
    if (selectedFiles.size === 0) return;
    setDownloading(true);
    setOutputPath("");
    setMessage(`正在下载 ${selectedFiles.size} 个归档文件...`);
    setShowModal(false);
    try {
      const summary = await invoke<DownloadSummary>("download_selected_archive_files", {
        fileUrls: Array.from(selectedFiles),
        outputPath: "",
      });
      if (summary.serverCount === 0) {
        setError("没有成功下载任何文件");
        setMessage("");
      } else {
        setOutputPath(summary.outputPath);
        setMessage(`已下载 ${summary.serverCount} 个文件，写入 ${formatBytes(summary.bytesWritten)}`);
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
    setMessage(`正在截取 ${enabledEntries.length} 条日志的最后 ${count} 行...`);
    try {
      const summary = await invoke<DownloadSummary>("download_tail_logs", {
        logEntryIds: enabledEntries.map((e) => e.id),
        lineCount: tailLineCount,
        outputPath: "",
      });
      setOutputPath(summary.outputPath);
      setMessage(`已截取 ${summary.serverCount} 条日志各 ${count} 行，写入 ${formatBytes(summary.bytesWritten)}`);
    } catch (caught) {
      setError(String(caught));
      setMessage("");
    } finally {
      setDownloading(false);
    }
  };

  const handleOpenFile = async () => {
    if (!outputPath) return;
    try { await invoke("open_file", { path: outputPath }); } catch (caught) { setError(String(caught)); }
  };

  const handleOpenFolder = async () => {
    if (!outputPath) return;
    try { await invoke("open_folder", { path: outputPath }); } catch (caught) { setError(String(caught)); }
  };

  const handleCopyPrompt = async () => {
    if (!outputPath) return;
    try {
      await invoke("copy_agent_prompt", { filePath: outputPath });
      setMessage("提示词已复制到剪切板，请打开 ai工具 粘贴");
    } catch (caught) { setError(String(caught)); }
  };

  const fileCount = archiveFiles.filter((f) => !f.isDir).length;
  const allSelected = selectedFiles.size === fileCount && fileCount > 0;
  const someSelected = selectedFiles.size > 0 && selectedFiles.size < fileCount;

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-y-auto bg-[#f5f7fb] px-5 py-5">
      <div className="rounded-lg border border-[#d9e1ec] bg-white p-5">
        <div className="mb-4 flex items-center gap-4 border-b border-[#e3e8f0] pb-4">
          <button
            type="button"
            className={`inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md ${
              mode === "realtime" ? "bg-[#2563eb] text-white" : "border border-[#cfd8e6] text-[#69778c] hover:bg-[#eef3f8]"
            }`}
            onClick={() => setMode("realtime")}
          >
            <Download size={16} /> 实时日志
          </button>
          <button
            type="button"
            className={`inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md ${
              mode === "archive" ? "bg-[#2563eb] text-white" : "border border-[#cfd8e6] text-[#69778c] hover:bg-[#eef3f8]"
            }`}
            onClick={() => setMode("archive")}
          >
            <Archive size={16} /> 归档日志
          </button>
          <button
            type="button"
            className={`inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md ${
              mode === "tail" ? "bg-[#2563eb] text-white" : "border border-[#cfd8e6] text-[#69778c] hover:bg-[#eef3f8]"
            }`}
            onClick={() => setMode("tail")}
          >
            <Scissors size={16} /> 截取日志
          </button>
        </div>

        <div className="mb-4 flex items-center gap-3">
          <span className="shrink-0 text-sm text-[#69778c]">下载目录</span>
          <div className="flex min-w-0 flex-1 items-center gap-2 rounded-md border border-[#cfd8e6] bg-[#f8fafc] px-3 py-2">
            <span className="truncate text-sm text-[#243145]">
              {config.settings.downloadPath || "默认下载目录"}
            </span>
          </div>
          <button
            type="button"
            className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-[#cfd8e6] bg-white px-3 py-2 text-sm text-[#69778c] hover:bg-[#eef3f8]"
            onClick={async () => {
              const dir = await pickDirectory();
              if (dir) {
                const updated = { ...config, settings: { ...config.settings, downloadPath: dir } };
                await useServerStore.getState().save(updated);
              }
            }}
          >
            <FolderInput size={15} /> 选择目录
          </button>
        </div>

        {mode === "realtime" && (
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-base font-semibold text-[#243145]">下载实时日志</h3>
              <p className="mt-1 text-sm text-[#69778c]">下载勾选的日志文件（.gz 文件将自动解压）。</p>
            </div>
            <button
              className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-5 py-2.5 font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50"
              type="button"
              onClick={() => void downloadRealtime()}
              disabled={downloading || enabledEntries.length === 0}
            >
              <Download size={18} /> {downloading ? "下载中" : `下载 (${enabledEntries.length})`}
            </button>
          </div>
        )}

        {mode === "archive" && (
          <div>
            <h3 className="text-base font-semibold text-[#243145]">下载归档日志</h3>
            <p className="mt-1 text-sm text-[#69778c]">
              选择一个日志路径，点击加载后在弹窗中勾选文件并下载（.gz 文件将自动解压）。
            </p>
            <div className="mt-4 flex items-end gap-3">
              <label className="block text-sm font-medium">
                日志路径
                <select
                  className="mt-1 min-w-[200px] rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
                  value={archiveEntryId}
                  onChange={(e) => {
                    setArchiveEntryId(e.target.value);
                    setMessage("");
                    setError("");
                  }}
                >
                  <option value="">-- 请选择 --</option>
                  {enabledEntries.map((e) => (
                    <option key={e.id} value={e.id}>{e.name}</option>
                  ))}
                </select>
              </label>
              <button
                className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50"
                type="button"
                onClick={() => void openArchiveModal()}
                disabled={loadingFiles || downloading || !archiveEntryId}
              >
                <RefreshCw size={16} className={loadingFiles ? "animate-spin" : ""} /> {loadingFiles ? "加载中..." : "加载文件"}
              </button>
            </div>
          </div>
        )}

        {mode === "tail" && (
          <div>
            <h3 className="text-base font-semibold text-[#243145]">截取日志尾部</h3>
            <p className="mt-1 text-sm text-[#69778c]">获取勾选日志的最后 N 行（.gz 文件将自动解压）。</p>
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
                disabled={downloading || enabledEntries.length === 0}
              >
                <Scissors size={18} /> {downloading ? "截取中" : `截取 (${enabledEntries.length})`}
              </button>
            </div>
          </div>
        )}

        {message && (
          <div className="mt-4 flex items-center gap-3 rounded-md bg-[#ecfdf3] px-3 py-2">
            <span className="text-sm text-[#047857]">{message}</span>
            {outputPath && (
              <div className="flex gap-2">
                <button type="button" onClick={() => void handleOpenFile()} className="inline-flex items-center gap-1 rounded border border-[#047857] px-2 py-1 text-xs text-[#047857] hover:bg-[#047857] hover:text-white">
                  <FileText size={14} /> 打开文件
                </button>
                <button type="button" onClick={() => void handleOpenFolder()} className="inline-flex items-center gap-1 rounded border border-[#047857] px-2 py-1 text-xs text-[#047857] hover:bg-[#047857] hover:text-white">
                  <FolderOpen size={14} /> 打开文件夹
                </button>
                <button type="button" onClick={() => void handleCopyPrompt()} className="inline-flex items-center gap-1 rounded border border-[#7c3aed] px-2 py-1 text-xs text-[#7c3aed] hover:bg-[#7c3aed] hover:text-white">
                  <Copy size={14} /> 复制提示词
                </button>
              </div>
            )}
          </div>
        )}
        {error && <div className="mt-4 rounded-md bg-[#fff1f3] px-3 py-2 text-sm text-[#b42318]">{error}</div>}
      </div>

      {/* Archive file picker modal */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" onClick={() => setShowModal(false)}>
          <div className="flex w-[560px] max-h-[80vh] flex-col rounded-lg bg-white shadow-xl" onClick={(e) => e.stopPropagation()}>
            {/* Header */}
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

            {/* Select all bar */}
            <div className="flex items-center justify-between border-b border-[#e8ecf2] px-5 py-2">
              <label className="inline-flex cursor-pointer items-center gap-2 text-sm text-[#243145]">
                <input className="sr-only" type="checkbox" checked={allSelected} onChange={toggleAllFiles} />
                <span
                  className={`flex h-4 w-4 items-center justify-center rounded border ${
                    allSelected ? "border-[#2563eb] bg-[#2563eb]" : someSelected ? "border-[#2563eb] bg-[#2563eb]" : "border-[#cfd8e6] bg-white"
                  }`}
                  onClick={toggleAllFiles}
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

            {/* File list */}
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
                    <label
                      key={file.url}
                      className="flex cursor-pointer items-center gap-2.5 rounded px-4 py-2 text-sm hover:bg-[#f0f4ff]"
                    >
                      <input className="sr-only" type="checkbox" checked={selectedFiles.has(file.url)} onChange={() => toggleFile(file.url)} />
                      <CustomCheckbox checked={selectedFiles.has(file.url)} onChange={() => toggleFile(file.url)} />
                      <span className="flex-1 truncate text-[#243145]">{file.name}</span>
                      {file.name.endsWith(".gz") && (
                        <span className="shrink-0 rounded bg-[#dbeafe] px-1.5 py-0.5 text-[10px] font-medium text-[#2563eb]">gz</span>
                      )}
                    </label>
                  )
                )
              )}
            </div>

            {/* Footer */}
            <div className="flex items-center justify-end gap-3 border-t border-[#e3e8f0] px-5 py-3">
              <button
                className="rounded-md border border-[#cfd8e6] bg-white px-4 py-2 text-sm text-[#69778c] hover:bg-[#f3f4f6]"
                onClick={() => setShowModal(false)}
              >
                取消
              </button>
              <button
                className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-5 py-2 text-sm font-medium text-white hover:bg-[#1d4ed8] disabled:opacity-50"
                onClick={() => void downloadSelectedArchive()}
                disabled={downloading || selectedFiles.size === 0}
              >
                <Download size={16} /> 下载选中 ({selectedFiles.size})
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
