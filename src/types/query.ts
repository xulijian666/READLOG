import type { LogEvent, LogLevel } from "./log";

export interface QueryRequest {
  queryId: string;
  logEntryIds: string[];
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

export interface LogEntry {
  id: string;
  name: string;
  path: string;
  logFile: string;
  visible: boolean;
  enabled: boolean;
  displayOrder: number;
  groupId: string;
  groupName: string;
}

export interface ServerConfig {
  id: string;
  name: string;
  baseUrl: string;
  enabled: boolean;
  displayOrder: number;
}

export interface AppConfig {
  baseUrl: string;
  logEntries: LogEntry[];
  credentials: {
    username: string;
    password: string;
  };
  settings: {
    maxConcurrentServers: number;
    defaultBatchSize: number;
    defaultLevel: LogLevel;
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
  logEntryId: string;
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
