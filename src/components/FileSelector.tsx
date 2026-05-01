import { RefreshCcw } from "lucide-react";
import { invoke } from "../lib/runtime";
import { useState } from "react";
import { useQueryStore } from "../store/queryStore";
import type { DirEntry, ServerConfig } from "../types/query";

interface Props {
  servers: ServerConfig[];
}

export function FileSelector({ servers }: Props) {
  const { filePath, setFilter } = useQueryStore();
  const [files, setFiles] = useState<DirEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const enabled = servers.find((server) => server.enabled);

  const refresh = async () => {
    if (!enabled) return;
    setLoading(true);
    try {
      const entries = await invoke<DirEntry[]>("list_directory", { serverId: enabled.id });
      setFiles(entries.filter((entry) => !entry.isDir));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex min-w-[260px] items-end gap-2">
      <label className="flex-1 text-sm font-medium">
        文件
        <input
          className="mt-1 w-full rounded-md border border-[#cfd8e6] px-3 py-2 outline-none focus:border-[#2563eb]"
          list="log-files"
          value={filePath}
          onChange={(event) => setFilter({ filePath: event.target.value })}
        />
      </label>
      <datalist id="log-files">
        {files.map((file) => (
          <option key={file.url} value={file.name} />
        ))}
      </datalist>
      <button
        className="rounded-md border border-[#cfd8e6] p-2.5 text-[#4f6177] hover:bg-[#eef3f8] disabled:opacity-50"
        type="button"
        onClick={() => void refresh()}
        disabled={!enabled || loading}
        aria-label="刷新文件"
        title="刷新文件"
      >
        <RefreshCcw size={17} className={loading ? "animate-spin" : ""} />
      </button>
    </div>
  );
}
