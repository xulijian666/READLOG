import { CheckCircle2, ChevronDown, ChevronRight, ExternalLink, PlugZap, Save, Search, Settings, XCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { invoke } from "../lib/runtime";
import { useServerStore } from "../store/serverStore";
import type { ConnectionCheckResult } from "../types/query";
import { LogPathModal } from "./LogPathModal";

type CheckState = Record<string, { loading: boolean; result?: ConnectionCheckResult; error?: string }>;
type ContextMenuState = { x: number; y: number; entryId: string; dirUrl: string } | null;

export function ServerConfig() {
  const { config, toggleLogEntry, toggleGroup, save } = useServerStore();
  const [showModal, setShowModal] = useState(false);
  const [authDraft, setAuthDraft] = useState({ username: "", password: "" });
  const [baseUrlDraft, setBaseUrlDraft] = useState("");
  const [dirty, setDirty] = useState(false);
  const [checkingAll, setCheckingAll] = useState(false);
  const [checks, setChecks] = useState<CheckState>({});
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [searchText, setSearchText] = useState("");
  const [searchInput, setSearchInput] = useState("");
  const savedEnabledRef = useRef<Record<string, boolean> | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenuState>(null);
  const sectionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!config || dirty) return;
    setAuthDraft(config.credentials);
    setBaseUrlDraft(config.baseUrl);
    // Auto-expand all groups
    const groupIds = new Set(config.logEntries.filter((e) => e.groupId).map((e) => e.groupId));
    setExpandedGroups(groupIds);
  }, [dirty, config]);

  // Refs for indeterminate checkboxes
  const groupCheckboxRefs = useRef<Record<string, HTMLInputElement | null>>({});

  useEffect(() => {
    if (!config) return;
    const groups = new Map<string, typeof config.logEntries>();
    for (const entry of config.logEntries) {
      if (!entry.groupId) continue;
      if (!groups.has(entry.groupId)) groups.set(entry.groupId, []);
      groups.get(entry.groupId)!.push(entry);
    }
    for (const [groupId, entries] of groups) {
      const ref = groupCheckboxRefs.current[groupId];
      if (!ref) continue;
      const enabledCount = entries.filter((e) => e.enabled).length;
      ref.indeterminate = enabledCount > 0 && enabledCount < entries.length;
    }
  });

  if (!config) return null;

  const visibleEntries = config.logEntries.filter((e) => e.visible);

  // Build groups
  const groupMap = new Map<string, { name: string; entries: typeof config.logEntries }>();
  const ungrouped: typeof config.logEntries = [];
  for (const entry of visibleEntries) {
    if (!entry.groupId) {
      ungrouped.push(entry);
      continue;
    }
    if (!groupMap.has(entry.groupId)) {
      groupMap.set(entry.groupId, { name: entry.groupName, entries: [] });
    }
    groupMap.get(entry.groupId)!.entries.push(entry);
  }

  const toggleExpand = (groupId: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) next.delete(groupId);
      else next.add(groupId);
      return next;
    });
  };

  const matchEntry = (entry: typeof visibleEntries[0], text: string) => {
    const lower = text.toLowerCase();
    return entry.name.toLowerCase().includes(lower) || entry.path.toLowerCase().includes(lower);
  };

  const handleSearch = (text: string) => {
    if (!searchText && text && !savedEnabledRef.current) {
      savedEnabledRef.current = Object.fromEntries(
        config.logEntries.map((e) => [e.id, e.enabled])
      );
    }

    setSearchText(text);

    if (text) {
      if (savedEnabledRef.current) {
        for (const entry of config.logEntries) {
          if (entry.enabled !== savedEnabledRef.current[entry.id]) {
            toggleLogEntry(entry.id);
          }
        }
      }
      for (const entry of config.logEntries) {
        if (matchEntry(entry, text) && !entry.enabled) {
          toggleLogEntry(entry.id);
        }
      }
    } else {
      if (savedEnabledRef.current) {
        for (const entry of config.logEntries) {
          if (entry.enabled !== savedEnabledRef.current[entry.id]) {
            toggleLogEntry(entry.id);
          }
        }
        savedEnabledRef.current = null;
      }
    }
  };

  const triggerSearch = () => {
    handleSearch(searchInput.trim());
  };

  const clearSearch = () => {
    setSearchInput("");
    handleSearch("");
  };

  const filteredGroupMap = searchText
    ? new Map([...groupMap].filter(([, group]) =>
        group.entries.some((e) => matchEntry(e, searchText))
      ))
    : groupMap;

  const filteredUngrouped = searchText
    ? ungrouped.filter((e) => matchEntry(e, searchText))
    : ungrouped;

  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, [contextMenu]);

  const saveAll = async () => {
    await save({
      ...config,
      baseUrl: baseUrlDraft.endsWith("/") ? baseUrlDraft : baseUrlDraft + "/",
      credentials: authDraft,
    });
    setDirty(false);
  };

  const testAll = async () => {
    setCheckingAll(true);
    const ids = visibleEntries.map((e) => e.id);
    setChecks(Object.fromEntries(ids.map((id) => [id, { loading: true }])));
    try {
      const results = await invoke<ConnectionCheckResult[]>("test_all_connections", {
        logEntryIds: ids,
      });
      setChecks(Object.fromEntries(results.map((r) => [r.logEntryId, { loading: false, result: r }])));
    } catch (error) {
      setChecks(Object.fromEntries(ids.map((id) => [id, { loading: false, error: String(error) }])));
    } finally {
      setCheckingAll(false);
    }
  };

  const renderCheckbox = (entry: typeof visibleEntries[0]) => {
    const check = checks[entry.id];
    const fullUrl = config.baseUrl.replace(/\/+$/, "") + "/" + entry.path.replace(/^\/+/, "") + entry.logFile;
    return (
      <label
        key={entry.id}
        title={fullUrl}
        onContextMenu={(e) => {
          e.preventDefault();
          setContextMenu({ x: e.clientX, y: e.clientY, entryId: entry.id, dirUrl: config.baseUrl.replace(/\/+$/, "") + "/" + entry.path.replace(/^\/+/, "") });
        }}
        className={`inline-flex cursor-pointer items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs transition-colors ${
          entry.enabled
            ? "border-[#2563eb] bg-[#eff4ff] text-[#2563eb]"
            : "border-[#d7dfeb] bg-[#f8fafc] text-[#69778c]"
        }`}
      >
        <input
          className="sr-only"
          type="checkbox"
          checked={entry.enabled}
          onChange={() => void toggleLogEntry(entry.id)}
        />
        <span
          className={`flex h-3.5 w-3.5 items-center justify-center rounded border ${
            entry.enabled ? "border-[#2563eb] bg-[#2563eb]" : "border-[#cfd8e6] bg-white"
          }`}
        >
          {entry.enabled && (
            <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
              <path d="M1 4L3.5 6.5L9 1" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          )}
        </span>
        {entry.name}
        {check?.loading && <span className="ml-1 text-[#607086]">...</span>}
        {check?.result && (
          <span className="ml-1">
            {check.result.ok ? <CheckCircle2 size={13} className="text-[#047857]" /> : <XCircle size={13} className="text-[#b42318]" />}
          </span>
        )}
        {check?.error && <XCircle size={13} className="ml-1 text-[#b42318]" />}
      </label>
    );
  };

  return (
    <section ref={sectionRef} className="relative border-b border-[#d9e1ec] bg-white px-5 py-4">
      {/* Top row: credentials + base URL */}
      <div className="mb-3 grid max-w-[1080px] items-end gap-4" style={{ gridTemplateColumns: "1fr 1fr 2fr auto auto" }}>
        <label className="block text-sm font-medium">
          用户名
          <input
            className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
            value={authDraft.username}
            onChange={(e) => {
              setDirty(true);
              setAuthDraft((d) => ({ ...d, username: e.target.value }));
            }}
          />
        </label>
        <label className="block text-sm font-medium">
          密码
          <input
            className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
            type="password"
            value={authDraft.password}
            onChange={(e) => {
              setDirty(true);
              setAuthDraft((d) => ({ ...d, password: e.target.value }));
            }}
          />
        </label>
        <label className="block text-sm font-medium">
          URL 前缀
          <input
            className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
            value={baseUrlDraft}
            placeholder="http://10.142.149.25:61000/"
            onChange={(e) => {
              setDirty(true);
              setBaseUrlDraft(e.target.value);
            }}
          />
        </label>
        <button
          className="inline-flex items-center gap-2 rounded-md bg-[#10b981] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#059669] disabled:opacity-50"
          type="button"
          disabled={!dirty}
          onClick={() => void saveAll()}
        >
          <Save size={16} /> 保存
        </button>
        <button
          className="inline-flex items-center gap-2 rounded-md border border-[#cfd8e6] bg-white px-4 py-2.5 text-sm font-medium hover:bg-[#eef3f8] disabled:opacity-50"
          type="button"
          disabled={checkingAll || dirty || visibleEntries.length === 0}
          onClick={() => void testAll()}
          title={dirty ? "请先保存配置" : "测试所有日志连接"}
        >
          <PlugZap size={16} /> {checkingAll ? "测试中" : "测试连接"}
        </button>
      </div>

      {/* Log entry checkboxes - grouped */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <button
            className="inline-flex items-center gap-1.5 rounded-md border border-[#cfd8e6] bg-[#f8fafc] px-3 py-1.5 text-xs font-medium text-[#69778c] hover:bg-[#eef3f8]"
            type="button"
            onClick={() => setShowModal(true)}
          >
            <Settings size={14} /> 日志路径配置
          </button>
          <div className="relative">
            <input
              className="w-64 rounded-md border border-[#cfd8e6] bg-white py-1.5 pl-3 pr-7 text-xs outline-none focus:border-[#2563eb]"
              placeholder="搜索日志 URL 或文件名..."
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") triggerSearch(); }}
            />
            {searchInput && (
              <button
                className="absolute right-2 top-1/2 -translate-y-1/2 text-[#9ca3af] hover:text-[#243145]"
                type="button"
                onClick={clearSearch}
              >
                <XCircle size={14} />
              </button>
            )}
          </div>
          <button
            className="inline-flex items-center gap-1.5 rounded-md bg-[#2563eb] px-3 py-1.5 text-xs font-medium text-white hover:bg-[#1d4ed8]"
            type="button"
            onClick={triggerSearch}
          >
            <Search size={14} /> 搜索
          </button>
        </div>
        {/* Groups */}
        {Array.from(filteredGroupMap.entries()).map(([groupId, group]) => {
          const allEnabled = group.entries.length > 0 && group.entries.every((e) => e.enabled);
          const someEnabled = group.entries.some((e) => e.enabled);
          const isExpanded = expandedGroups.has(groupId);

          return (
            <div key={groupId}>
              <div className="flex items-center gap-2">
                <button
                  className="rounded p-0.5 text-[#5d6b82] hover:bg-[#eef3f8]"
                  type="button"
                  onClick={() => toggleExpand(groupId)}
                >
                  {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </button>
                <label className="inline-flex cursor-pointer items-center gap-1.5 text-xs font-medium text-[#243145]">
                  <input
                    ref={(el) => { groupCheckboxRefs.current[groupId] = el; }}
                    className="sr-only"
                    type="checkbox"
                    checked={allEnabled}
                    onChange={() => void toggleGroup(groupId)}
                  />
                  <span
                    className={`flex h-3.5 w-3.5 items-center justify-center rounded border ${
                      allEnabled
                        ? "border-[#2563eb] bg-[#2563eb]"
                        : someEnabled
                          ? "border-[#2563eb] bg-[#2563eb]"
                          : "border-[#cfd8e6] bg-white"
                    }`}
                  >
                    {allEnabled && (
                      <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
                        <path d="M1 4L3.5 6.5L9 1" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                    )}
                    {someEnabled && !allEnabled && (
                      <svg width="10" height="2" viewBox="0 0 10 2" fill="none">
                        <path d="M1 1H9" stroke="white" strokeWidth="1.5" strokeLinecap="round" />
                      </svg>
                    )}
                  </span>
                  {group.name}
                </label>
              </div>
              {isExpanded && (
                <div className="mt-1.5 flex flex-wrap gap-2 pl-7">
                  {group.entries.map(renderCheckbox)}
                </div>
              )}
            </div>
          );
        })}

        {/* Ungrouped */}
        {filteredUngrouped.length > 0 && (
          <div className="flex flex-wrap items-center gap-2">
            {filteredUngrouped.map(renderCheckbox)}
          </div>
        )}

        {visibleEntries.length === 0 && (
          <span className="text-xs text-[#9ca3af]">暂无日志，请先配置日志路径</span>
        )}
      </div>

      <p className="mt-3 text-xs text-[#69778c]">
        勾选要操作的日志，下载和截取时将直接使用配置的日志文件 URL（{baseUrlDraft}路径/日志文件）。若日志文件为 .gz 格式将自动解压。
      </p>

      {contextMenu && (
        <div
          className="fixed z-50 min-w-[120px] rounded-md border border-[#d9e1ec] bg-white py-1 shadow-lg"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          <button
            className="flex w-full items-center gap-2 px-3 py-1.5 text-xs text-[#243145] hover:bg-[#f0f4ff]"
            type="button"
            onClick={() => {
              void invoke("open_file", { path: contextMenu.dirUrl });
              setContextMenu(null);
            }}
          >
            <ExternalLink size={13} /> 跳转
          </button>
        </div>
      )}

      {showModal && <LogPathModal onClose={() => setShowModal(false)} />}
    </section>
  );
}
