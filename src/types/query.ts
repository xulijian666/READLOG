import type { LogEvent, LogLevel } from "./log";

export interface QueryRequest {
  queryId: string;
  serverIds: string[];
  filePath: string;
  startTime: string | null;
  endTime: string | null;
  keyword: string;
  level: LogLevel;
  batchSize: number;
}

export interface QueryProgressEvent {
  queryId: string;
  status: "running" | "completed" | "cancelled" | string;
  scannedBytes: number;
  scannedEvents: number;
  matchedEvents: number;
  serversCompleted: string[];
  serversPending: string[];
}

export interface QueryResultEvent {
  queryId: string;
  batchIndex: number;
  events: LogEvent[];
  isLastBatch: boolean;
}

export interface ServerConfig {
  id: string;
  name: string;
  baseUrl: string;
  enabled: boolean;
  displayOrder: number;
}

export interface AppConfig {
  servers: ServerConfig[];
  credentials: {
    username: string;
    password: string;
  };
  settings: {
    maxConcurrentServers: number;
    defaultBatchSize: number;
    defaultLevel: LogLevel;
    logType: string;
    downloadPath: string;
  };
}

export interface DirEntry {
  name: string;
  url: string;
  isDir: boolean;
}

export interface ConnectionCheckResult {
  ok: boolean;
  serverId: string;
  serverName: string;
  statusCode: number;
  message: string;
  fileCount: number;
  fileSize?: number | null;
}

export interface DownloadSummary {
  serverCount: number;
  bytesWritten: number;
  outputPath: string;
}
