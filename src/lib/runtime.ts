import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { save as tauriSave } from "@tauri-apps/plugin-dialog";
import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import type { AppConfig, ConnectionCheckResult, DirEntry, DownloadSummary } from "../types/query";

export function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const mockConfig: AppConfig = {
  servers: [
    {
      id: "mock-sit-124",
      name: "SIT-124",
      baseUrl: "http://10.142.149.25:61000/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.124/",
      enabled: true,
      displayOrder: 0,
    },
    {
      id: "mock-sit-186",
      name: "SIT-186",
      baseUrl: "http://10.142.149.25:61000/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.186/",
      enabled: true,
      displayOrder: 1,
    },
    {
      id: "mock-sit-50",
      name: "SIT-50",
      baseUrl: "http://10.142.149.25:61000/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.50/",
      enabled: false,
      displayOrder: 2,
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
    logType: "app",
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
      return readBrowserConfig().servers
        .filter((server) => (args?.serverIds as string[] | undefined)?.includes(server.id) ?? true)
        .map(
          (server) =>
            ({
              ok: true,
              serverId: server.id,
              serverName: server.name,
              statusCode: 200,
              message: "浏览器预览连接正常",
              fileCount: 1,
              fileSize: 1024 * 1024,
            }) satisfies ConnectionCheckResult,
        ) as T;
    case "download_checked_logs":
      return {
        serverCount: (args?.serverIds as string[] | undefined)?.length ?? 0,
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
