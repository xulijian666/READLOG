import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { save as tauriSave } from "@tauri-apps/plugin-dialog";
import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import type { AppConfig, ConnectionCheckResult, DirEntry, DownloadSummary } from "../types/query";

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
      return {
        serverCount: (args?.logEntryIds as string[] | undefined)?.length ?? 0,
        bytesWritten: 0,
        outputPath: String(args?.outputPath ?? ""),
      } satisfies DownloadSummary as T;
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
