export type LogLevel = "ALL" | "DEBUG" | "INFO" | "WARN" | "ERROR";

export interface HighlightRange {
  start: number;
  end: number;
}

export interface LogEvent {
  id: string;
  serverId: string;
  serverName: string;
  serverDisplayOrder: number;
  lineOffset: number;
  timestamp: string;
  level: string;
  thread: string;
  className: string;
  firstLineContent: string;
  rawText: string;
  lineCount: number;
  highlighted: boolean;
  highlightRanges: HighlightRange[];
}
