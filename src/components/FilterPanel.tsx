import { Download, Search, XCircle } from "lucide-react";
import { invoke, saveDialog } from "../lib/runtime";
import { useQueryStore } from "../store/queryStore";
import { useQuery } from "../hooks/useQuery";
import type { ServerConfig } from "../types/query";
import { FileSelector } from "./FileSelector";

interface Props {
  servers: ServerConfig[];
  batchSize: number;
}

export function FilterPanel({ servers, batchSize }: Props) {
  const query = useQueryStore();
  const { execute, cancel } = useQuery();
  const enabledServers = servers.filter((server) => server.enabled);

  const downloadOriginal = async () => {
    const server = enabledServers[0];
    if (!server) return;
    const outputPath = await saveDialog({ defaultPath: query.filePath });
    if (!outputPath) return;
    await invoke("download_original_file", { serverId: server.id, filePath: query.filePath, outputPath });
  };

  return (
    <section className="border-b border-[#d9e1ec] bg-[#fbfcfe] px-5 py-4">
      <div className="mb-3 flex items-center justify-between">
        <h2 className="text-sm font-semibold text-[#243145]">筛选</h2>
        <button
          className="inline-flex items-center gap-2 rounded-md border border-[#cfd8e6] px-3 py-2 text-sm hover:bg-[#eef3f8] disabled:opacity-50"
          type="button"
          onClick={() => void downloadOriginal()}
          disabled={!enabledServers.length || !query.filePath}
        >
          <Download size={16} /> 下载原文件
        </button>
      </div>
      <div className="grid gap-4" style={{ gridTemplateColumns: "1.2fr 1fr 1fr 0.8fr auto" }}>
        <FileSelector servers={servers} />
        <label className="text-sm font-medium">
          开始时间
          <input
            className="mt-1 w-full rounded-md border border-[#cfd8e6] px-3 py-2 outline-none focus:border-[#2563eb]"
            type="datetime-local"
            value={query.startTime}
            onChange={(event) => query.setFilter({ startTime: event.target.value })}
          />
        </label>
        <label className="text-sm font-medium">
          结束时间
          <input
            className="mt-1 w-full rounded-md border border-[#cfd8e6] px-3 py-2 outline-none focus:border-[#2563eb]"
            type="datetime-local"
            value={query.endTime}
            onChange={(event) => query.setFilter({ endTime: event.target.value })}
          />
        </label>
        <label className="text-sm font-medium">
          级别
          <select
            className="mt-1 w-full rounded-md border border-[#cfd8e6] px-3 py-2 outline-none focus:border-[#2563eb]"
            value={query.level}
            onChange={(event) => query.setFilter({ level: event.target.value as typeof query.level })}
          >
            <option value="ALL">全部</option>
            <option value="DEBUG">DEBUG</option>
            <option value="INFO">INFO</option>
            <option value="WARN">WARN</option>
            <option value="ERROR">ERROR</option>
          </select>
        </label>
        <div className="flex items-end gap-2">
          <button
            className="inline-flex items-center gap-2 rounded-md bg-[#10b981] px-4 py-2.5 font-medium text-white hover:bg-[#059669] disabled:opacity-50"
            type="button"
            onClick={() => void execute(servers, batchSize)}
            disabled={!enabledServers.length || query.running}
          >
            <Search size={17} /> 搜索
          </button>
          <button
            className="inline-flex items-center gap-2 rounded-md border border-[#cfd8e6] px-4 py-2.5 hover:bg-[#eef3f8] disabled:opacity-50"
            type="button"
            onClick={() => void cancel()}
            disabled={!query.running}
          >
            <XCircle size={17} /> 取消
          </button>
        </div>
      </div>
      <label className="mt-4 block max-w-[520px] text-sm font-medium">
        关键词
        <input
          className="mt-1 w-full rounded-md border border-[#cfd8e6] px-3 py-2 outline-none focus:border-[#2563eb]"
          value={query.keyword}
          onChange={(event) => query.setFilter({ keyword: event.target.value })}
        />
      </label>
    </section>
  );
}
