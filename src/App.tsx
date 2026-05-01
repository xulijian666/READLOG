import { useEffect } from "react";
import { AlertCircle } from "lucide-react";
import { DownloadPanel } from "./components/DownloadPanel";
import { ServerConfig } from "./components/ServerConfig";
import { useServerStore } from "./store/serverStore";

export default function App() {
  const { config, error, load, loading } = useServerStore();

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <main className="flex h-screen min-h-[640px] flex-col bg-[#f5f7fb] text-[#18212f]">
      <header className="flex h-14 items-center justify-between border-b border-[#d9e1ec] bg-white px-5">
        <div>
          <h1 className="text-lg font-semibold">日志下载器</h1>
          <p className="text-xs text-[#6b778c]">ReadLog Downloader</p>
        </div>
        <div className="text-sm text-[#607086]">{loading ? "加载中" : "Windows 桌面版"}</div>
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
          <DownloadPanel />
        </>
      )}
    </main>
  );
}
