import type { LogEntry } from "../types/query";

const nameOrder = new Intl.Collator("zh-CN", { numeric: true, sensitivity: "base" });

export function sortLogEntries(entries: LogEntry[]): LogEntry[] {
  return [...entries].sort((a, b) => nameOrder.compare(a.name, b.name) || nameOrder.compare(a.path, b.path) || a.id.localeCompare(b.id));
}

export function groupLogEntries(entries: LogEntry[]) {
  const groups = new Map<string, { name: string; entries: LogEntry[] }>();
  for (const entry of sortLogEntries(entries)) {
    const id = entry.groupId;
    if (!groups.has(id)) groups.set(id, { name: id ? entry.groupName : "未分组", entries: [] });
    groups.get(id)!.entries.push(entry);
  }
  return [...groups.entries()].sort(([aId, a], [bId, b]) => {
    if (!aId) return 1;
    if (!bId) return -1;
    return nameOrder.compare(a.name, b.name);
  });
}

export function matchesLogEntry(entry: LogEntry, query: string, baseUrl: string): boolean {
  const text = `${entry.name} ${entry.groupName} ${baseUrl.replace(/\/+$/, "")}/${entry.path.replace(/^\/+/, "")}${entry.logFile}`.toLocaleLowerCase();
  return query.trim().toLocaleLowerCase().split(/\s+/).every(term => text.includes(term));
}
