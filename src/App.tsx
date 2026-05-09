import { useEffect, useState } from "react";
import { AlertCircle, Download, Search } from "lucide-react";
import { DownloadPanel } from "./components/DownloadPanel";
import { SearchPanel } from "./components/SearchPanel";
import { ServerConfig } from "./components/ServerConfig";
import { useServerStore } from "./store/serverStore";

type ActivePanel = "download" | "search";

export default function App() {
  const { config, error, load, loading } = useServerStore();
  const [activePanel, setActivePanel] = useState<ActivePanel>("download");

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <main className="flex h-screen min-h-[640px] flex-col bg-[#f5f7fb] text-[#18212f]">
      <header className="flex h-14 items-center justify-between border-b border-[#d9e1ec] bg-white px-5">
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
          <div className="text-sm text-[#607086]">{loading ? "加载中" : "Windows 桌面版"}</div>
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
          <ServerConfig />
          {activePanel === "download" ? <DownloadPanel /> : <SearchPanel />}
        </>
      )}
    </main>
  );
}
