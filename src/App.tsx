import { useEffect, useRef, useState } from "react";
import { AlertCircle, Copy, Download, Search, Settings, X } from "lucide-react";
import { DownloadPanel } from "./components/DownloadPanel";
import { SearchPanel } from "./components/SearchPanel";
import { ServerConfig } from "./components/ServerConfig";
import { invoke, isTauriRuntime } from "./lib/runtime";
import { useServerStore } from "./store/serverStore";

type ActivePanel = "download" | "search";

export default function App() {
  const { config, error, load, loading } = useServerStore();
  const [activePanel, setActivePanel] = useState<ActivePanel>("download");
  const [drawer, setDrawer] = useState<"connection" | "scope" | null>(null);
  const [copyingPrompt, setCopyingPrompt] = useState(false);
  const [copyResult, setCopyResult] = useState<{ ok: boolean; message: string } | null>(null);
  const drawerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!drawer) return;
    const previous = document.activeElement as HTMLElement | null;
    drawerRef.current?.querySelector<HTMLButtonElement>("button")?.focus();
    return () => previous?.focus();
  }, [drawer]);

  useEffect(() => {
    void load();
  }, [load]);

  const copySkillPrompt = async () => {
    setCopyingPrompt(true);
    setCopyResult(null);
    try {
      if (!isTauriRuntime()) throw new Error("请在 Windows 桌面应用中使用此功能。");
      await invoke("copy_skill_install_prompt");
      setCopyResult({ ok: true, message: "已复制。把提示词原样粘贴给能在本机执行命令的 AI（如 Claude Code 或 Codex），装好后直接问它日志问题。" });
    } catch (caught) {
      setCopyResult({ ok: false, message: `复制失败：${String(caught)}` });
    } finally {
      setCopyingPrompt(false);
    }
  };

  return (
    <main className="flex h-screen min-h-0 flex-col overflow-hidden bg-[#f5f7fb] text-[#18212f]">
      <header className="flex h-12 shrink-0 items-center justify-between border-b border-[#d9e1ec] bg-white px-5">
        <div>
          <h1 className="text-lg font-semibold">日志工具</h1>
          <p className="text-xs text-[#6b778c]">ReadLog</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="inline-flex rounded-md border border-[#cfd8e6] bg-[#f8fafc] p-1">
            <button
              type="button"
              className={`inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-sm ${
                activePanel === "download" ? "bg-white text-[#2563eb] shadow-sm" : "text-[#607086]"
              }`}
              onClick={() => setActivePanel("download")}
            >
              <Download size={15} /> 下载日志
            </button>
            <button
              type="button"
              className={`inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-sm ${
                activePanel === "search" ? "bg-white text-[#2563eb] shadow-sm" : "text-[#607086]"
              }`}
              onClick={() => setActivePanel("search")}
            >
              <Search size={15} /> 查询日志
            </button>
          </div>
          {activePanel === "search" && <button type="button" className="reader-tool" onClick={() => setDrawer("connection")}><Settings size={14} />连接配置</button>}
          <button
            type="button"
            disabled={copyingPrompt}
            onClick={() => void copySkillPrompt()}
            title="把安装提示词粘贴给本机 AI，让它能查询日志"
            className="inline-flex shrink-0 items-center gap-1.5 rounded-md bg-[#2563eb] px-3 py-2 text-sm font-medium text-white hover:bg-[#1d4ed8] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#2563eb] disabled:opacity-60"
          >
            <Copy size={15} aria-hidden="true" /> {copyingPrompt ? "正在复制…" : "复制提示词给 AI"}
          </button>
          <div className="text-xs text-[#607086]">{loading ? "加载中" : "Windows 桌面版"}</div>
        </div>
      </header>

      {copyResult && (
        <div className={`flex shrink-0 items-center justify-between gap-3 border-b px-5 py-2 text-sm ${copyResult.ok ? "border-[#bfdbfe] bg-[#eff6ff] text-[#1e40af]" : "border-[#fecdca] bg-[#fff1f3] text-[#b42318]"}`}>
          <span role={copyResult.ok ? "status" : "alert"}>{copyResult.message}</span>
          <button type="button" className="reader-tool" aria-label="关闭复制提示" onClick={() => setCopyResult(null)}><X size={16} aria-hidden="true" /></button>
        </div>
      )}

      {error && (
        <div className="flex items-center gap-2 border-b border-[#fecdca] bg-[#fff1f3] px-5 py-3 text-sm text-[#b42318]">
          <AlertCircle size={17} />
          {error}
        </div>
      )}

      {config && (
        <>
          <div className={activePanel === "download" ? "flex min-h-0 flex-1 flex-col overflow-hidden" : drawer ? "fixed inset-0 z-50 flex justify-end bg-black/30" : "hidden"} onClick={activePanel === "search" ? () => setDrawer(null) : undefined}>
            <div ref={drawerRef} role={activePanel === "search" ? "dialog" : undefined} aria-modal={activePanel === "search" ? true : undefined} aria-label={drawer === "connection" ? "连接配置" : "查询范围"}
              className={activePanel === "search" ? "flex h-full w-[560px] max-w-[85vw] flex-col bg-white shadow-xl" : "flex min-h-0 flex-1 flex-col"}
              onClick={event => event.stopPropagation()}
              onKeyDown={event => {
                if (activePanel !== "search") return;
                if (event.key === "Escape") { event.stopPropagation(); setDrawer(null); }
                if (event.key === "Tab") {
                  const items = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex="0"]')).filter(item => item.getClientRects().length > 0);
                  const first = items[0], last = items[items.length - 1];
                  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
                  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
                }
              }}>
              {activePanel === "search" && <div className="flex shrink-0 items-center justify-between border-b border-[#e3e8f0] px-5 py-4"><div><h2 className="text-sm font-semibold">{drawer === "connection" ? "连接配置" : "查询范围"}</h2><p className="mt-1 text-xs text-[#69778c]">{drawer === "connection" ? "配置认证信息与日志 URL 前缀" : "勾选需要查询的日志，关闭后立即应用"}</p></div><button type="button" className="reader-tool" aria-label="关闭配置面板" onClick={() => setDrawer(null)}><X size={18} /></button></div>}
              <div className={activePanel === "search" ? "min-h-0 flex-1 overflow-auto" : "flex min-h-0 flex-1 flex-col"}><ServerConfig view={activePanel === "download" ? "all" : drawer || "scope"} /></div>
            </div>
          </div>
          <div className={activePanel === "download" ? "flex max-h-[45vh] min-h-0 shrink-0 flex-col" : "hidden"}><DownloadPanel /></div>
          <div className={activePanel === "search" ? "flex min-h-0 flex-1 flex-col" : "hidden"}><SearchPanel onOpenScope={() => setDrawer("scope")} /></div>
        </>
      )}
    </main>
  );
}
