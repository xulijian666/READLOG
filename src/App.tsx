import { useEffect, useRef, useState } from "react";
import { AlertCircle, Download, Search, Settings, X } from "lucide-react";
import { DownloadPanel } from "./components/DownloadPanel";
import { SearchPanel } from "./components/SearchPanel";
import { ServerConfig } from "./components/ServerConfig";
import { useServerStore } from "./store/serverStore";

type ActivePanel = "download" | "search";

export default function App() {
  const { config, error, load, loading } = useServerStore();
  const [activePanel, setActivePanel] = useState<ActivePanel>("download");
  const [drawer, setDrawer] = useState<"connection" | "scope" | null>(null);
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
          <div className="text-xs text-[#607086]">{loading ? "加载中" : "Windows 桌面版"}</div>
        </div>
      </header>

      {error && (
        <div className="flex items-center gap-2 border-b border-[#fecdca] bg-[#fff1f3] px-5 py-3 text-sm text-[#b42318]">
          <AlertCircle size={17} />
          {error}
        </div>
      )}

      {config && (
        <>
          <div className={activePanel === "download" ? "max-h-[55vh] shrink-0 overflow-auto" : drawer ? "fixed inset-0 z-50 flex justify-end bg-black/30" : "hidden"} onClick={activePanel === "search" ? () => setDrawer(null) : undefined}>
            <div ref={drawerRef} role={activePanel === "search" ? "dialog" : undefined} aria-modal={activePanel === "search" ? true : undefined} aria-label={drawer === "connection" ? "连接配置" : "查询范围"}
              className={activePanel === "search" ? "flex h-full w-[560px] max-w-[85vw] flex-col bg-white shadow-xl" : ""}
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
              <div className={activePanel === "search" ? "min-h-0 flex-1 overflow-auto" : ""}><ServerConfig view={activePanel === "download" ? "all" : drawer || "scope"} /></div>
            </div>
          </div>
          <div className={activePanel === "download" ? "flex min-h-0 flex-1 flex-col" : "hidden"}><DownloadPanel /></div>
          <div className={activePanel === "search" ? "flex min-h-0 flex-1 flex-col" : "hidden"}><SearchPanel onOpenScope={() => setDrawer("scope")} /></div>
        </>
      )}
    </main>
  );
}
