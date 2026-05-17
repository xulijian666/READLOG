import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { save as tauriSave } from "@tauri-apps/plugin-dialog";
import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import type { AppConfig, ConnectionCheckResult, DirEntry, DownloadSummary, LogEntry } from "../types/query";

export function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const mockConfig: AppConfig = {
  baseUrl: "http://10.142.149.25:61000/",
  logEntries: [
    {
      id: "mock-sit-124",
      name: "SIT-124",
      path: "/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.124/",
      logFile: "app.log",
      visible: true,
      enabled: true,
      displayOrder: 0,
      groupId: "mock-g1",
      groupName: "核心服务",
    },
    {
      id: "mock-sit-186",
      name: "SIT-186",
      path: "/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.186/",
      logFile: "app.log",
      visible: true,
      enabled: true,
      displayOrder: 1,
      groupId: "mock-g1",
      groupName: "核心服务",
    },
    {
      id: "mock-sit-50",
      name: "SIT-50",
      path: "/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.50/",
      logFile: "app.log",
      visible: true,
      enabled: false,
      displayOrder: 2,
      groupId: "mock-g2",
      groupName: "接口服务",
    },
  ],
  credentials: {
    username: "cgisteam",
    password: "",
  },
  settings: {
    maxConcurrentServers: 3,
    defaultBatchSize: 500,
    defaultLevel: "ALL",
    downloadPath: "",
  },
};

function readBrowserConfig() {
  const raw = localStorage.getItem("readlog.config");
  if (!raw) return mockConfig;
  try {
    return JSON.parse(raw) as AppConfig;
  } catch {
    return mockConfig;
  }
}

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime()) {
    return tauriInvoke<T>(command, args);
  }

  switch (command) {
    case "load_app_config":
      return readBrowserConfig() as T;
    case "save_app_config": {
      const config = args?.config as AppConfig;
      localStorage.setItem("readlog.config", JSON.stringify(config));
      return config as T;
    }
    case "list_directory":
      return [
        { name: "app.log", url: "browser-preview://app.log", isDir: false },
      ] satisfies DirEntry[] as T;
    case "test_all_connections":
      return readBrowserConfig().logEntries
        .filter((entry) => (args?.logEntryIds as string[] | undefined)?.includes(entry.id) ?? true)
        .map(
          (entry) =>
            ({
              ok: true,
              logEntryId: entry.id,
              serverName: entry.name,
              statusCode: 200,
              message: "浏览器预览连接正常",
              fileCount: 1,
              fileSize: 1024 * 1024,
            }) satisfies ConnectionCheckResult,
        ) as T;
    case "download_realtime_logs":
    case "download_archive_logs":
    case "download_tail_logs":
    case "download_selected_archive_files":
      return {
        serverCount: (args?.logEntryIds as string[] | undefined)?.length ?? (args?.fileUrls as string[] | undefined)?.length ?? 0,
        bytesWritten: 0,
        outputPath: String(args?.outputPath ?? ""),
      } satisfies DownloadSummary as T;
    case "list_archive_files":
      return [
        { name: "app-2026-05-16_00.log.gz", url: "browser-preview://app-2026-05-16_00.log.gz", isDir: false },
        { name: "app-2026-05-16_01.log.gz", url: "browser-preview://app-2026-05-16_01.log.gz", isDir: false },
        { name: "app-2026-05-16_02.log", url: "browser-preview://app-2026-05-16_02.log", isDir: false },
      ] satisfies DirEntry[] as T;
    case "search_archive_files":
      return String(args?.request ? (args.request as Record<string, unknown>).queryId : "mock") as T;
    default:
      return undefined as T;
  }
}

export async function saveDialog(options: { defaultPath?: string }) {
  if (isTauriRuntime()) {
    return tauriSave(options);
  }
  return options.defaultPath ?? null;
}

export async function listen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  if (isTauriRuntime()) {
    return tauriListen(event, handler);
  }
  void event;
  void handler;
  return () => undefined;
}

export async function exportXlsx(entries: LogEntry[]): Promise<void> {
  const exportEntries = entries.map((e) => ({
    group_name: e.groupName,
    name: e.name,
    url: e.path + e.logFile,
  }));

  if (isTauriRuntime()) {
    const filePath = await tauriSave({
      defaultPath: "日志路径配置.xlsx",
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (filePath) {
      await tauriInvoke("export_xlsx", { entries: exportEntries, outputPath: filePath });
    }
    return;
  }

  // Browser fallback: use SheetJS dynamically
  const XLSX = await import("xlsx");
  const rows: string[][] = [["分组名称", "日志名称", "日志URL"]];
  for (const entry of entries) {
    rows.push([entry.groupName, entry.name, entry.path + entry.logFile]);
  }
  const ws = XLSX.utils.aoa_to_sheet(rows);
  ws["!cols"] = [{ wch: 20 }, { wch: 25 }, { wch: 80 }];
  const wb = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(wb, ws, "日志配置");
  XLSX.writeFile(wb, "日志路径配置.xlsx");
}

export async function pickDirectory(): Promise<string | null> {
  if (isTauriRuntime()) {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const result = await open({ directory: true, title: "选择下载目录" });
      if (result) return result;
    } catch {
      // Fallback to backend command
    }
    try {
      return await tauriInvoke<string | null>("pick_directory");
    } catch {
      return null;
    }
  }
  return null;
}
