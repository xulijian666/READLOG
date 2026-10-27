import { CheckCircle2, ChevronDown, ChevronRight, ExternalLink, PlugZap, Save, Search, Settings, X, XCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "../lib/runtime";
import { groupLogEntries, matchesLogEntry } from "../lib/serverEntries";
import { useServerStore } from "../store/serverStore";
import type { ConnectionCheckResult, LogEntry } from "../types/query";
import { LogPathModal } from "./LogPathModal";

type CheckState = Record<string, { loading: boolean; result?: ConnectionCheckResult; error?: string }>;
type ContextMenuState = { x: number; y: number; dirUrl: string } | null;

export function ServerConfig({ view = "all" }: { view?: "all" | "connection" | "scope" }) {
  const { config, toggleLogEntry, toggleLogEntries, save } = useServerStore();
  const [showModal, setShowModal] = useState(false);
  const [authDraft, setAuthDraft] = useState({ username: "", password: "" });
  const [baseUrlDraft, setBaseUrlDraft] = useState("");
  const [dirty, setDirty] = useState(false);
  const [checkingAll, setCheckingAll] = useState(false);
  const [checks, setChecks] = useState<CheckState>({});
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const [searchText, setSearchText] = useState("");
  const [contextMenu, setContextMenu] = useState<ContextMenuState>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!config || dirty) return;
    setAuthDraft(config.credentials);
    setBaseUrlDraft(config.baseUrl);
  }, [dirty, config]);

  useEffect(() => {
    if (!contextMenu) return;
    const dismissOutside = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) setContextMenu(null);
    };
    const dismiss = () => setContextMenu(null);
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") dismiss(); };
    document.addEventListener("pointerdown", dismissOutside, true);
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("scroll", dismiss, true);
    window.addEventListener("resize", dismiss);
    window.addEventListener("blur", dismiss);
    return () => {
      document.removeEventListener("pointerdown", dismissOutside, true);
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("scroll", dismiss, true);
      window.removeEventListener("resize", dismiss);
      window.removeEventListener("blur", dismiss);
    };
  }, [contextMenu]);

  useEffect(() => { setContextMenu(null); }, [view, showModal]);

  if (!config) return null;

  const visibleEntries = config.logEntries.filter(entry => entry.visible && entry.name);
  const filteredEntries = visibleEntries.filter(entry => matchesLogEntry(entry, searchText, config.baseUrl));
  const filteredIds = new Set(filteredEntries.map(entry => entry.id));
  const groups = groupLogEntries(filteredEntries);
  const selectedEntries = config.logEntries.filter(entry => entry.enabled && entry.visible && entry.name);
  const selectedOutsideFilter = selectedEntries.filter(entry => !filteredIds.has(entry.id)).length;
  const filtering = !!searchText.trim();
  const allCollapsed = groups.length > 0 && groups.every(([id]) => collapsedGroups.has(id));

  const toggleExpand = (groupId: string) => {
    setCollapsedGroups(prev => {
      const next = new Set(prev);
      if (next.has(groupId)) next.delete(groupId);
      else next.add(groupId);
      return next;
    });
  };

  const saveAll = async () => {
    await save({ ...config, baseUrl: baseUrlDraft.trim(), credentials: authDraft });
    if (!useServerStore.getState().error) setDirty(false);
  };

  const testAll = async () => {
    setCheckingAll(true);
    const ids = visibleEntries.map(entry => entry.id);
    setChecks(Object.fromEntries(ids.map(id => [id, { loading: true }])));
    try {
      const results = await invoke<ConnectionCheckResult[]>("test_all_connections", { logEntryIds: ids });
      setChecks(Object.fromEntries(results.map(result => [result.logEntryId, { loading: false, result }])));
    } catch (error) {
      setChecks(Object.fromEntries(ids.map(id => [id, { loading: false, error: String(error) }])));
    } finally {
      setCheckingAll(false);
    }
  };

  const renderEntry = (entry: LogEntry) => {
    const check = checks[entry.id];
    const dirUrl = config.baseUrl.replace(/\/+$/, "") + "/" + entry.path.replace(/^\/+/, "");
    return (
      <label key={entry.id} title={`${entry.name}\n${dirUrl}${entry.logFile}`}
        onContextMenu={event => {
          event.preventDefault();
          setContextMenu({ x: Math.max(8, Math.min(event.clientX, window.innerWidth - 152)), y: Math.max(8, Math.min(event.clientY, window.innerHeight - 52)), dirUrl });
        }}
        className={`server-entry ${entry.enabled ? "server-entry-selected" : ""}`}>
        <input type="checkbox" className="h-3.5 w-3.5 shrink-0 accent-[#2563eb]" checked={entry.enabled} onChange={() => void toggleLogEntry(entry.id)} />
        <span className="min-w-0 flex-1 truncate">{entry.name}</span>
        {check?.loading && <span className="shrink-0 text-[#607086]">...</span>}
        {check?.result && (check.result.ok ? <CheckCircle2 size={14} className="shrink-0 text-[#047857]" /> : <XCircle size={14} className="shrink-0 text-[#b42318]" />)}
        {check?.error && <XCircle size={14} className="shrink-0 text-[#b42318]" />}
      </label>
    );
  };

  return (
    <section className={`border-b border-[#d9e1ec] bg-white px-5 py-4 ${view === "all" ? "flex min-h-0 flex-1 flex-col overflow-hidden" : ""}`}>
      <div className={view === "scope" ? "hidden" : "mb-4 grid shrink-0 items-end gap-3"} style={{ gridTemplateColumns: view === "connection" ? "1fr 1fr" : "minmax(100px, 1fr) minmax(100px, 1fr) minmax(220px, 2fr) auto auto" }}>
        <label className="block min-w-0 text-sm font-medium">用户名
          <input className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]" value={authDraft.username}
            onChange={event => { setDirty(true); setAuthDraft(draft => ({ ...draft, username: event.target.value })); }} />
        </label>
        <label className="block min-w-0 text-sm font-medium">密码
          <input type="password" className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]" value={authDraft.password}
            onChange={event => { setDirty(true); setAuthDraft(draft => ({ ...draft, password: event.target.value })); }} />
        </label>
        <label className={`block min-w-0 text-sm font-medium ${view === "connection" ? "col-span-2" : ""}`}>URL 前缀
          <input className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]" value={baseUrlDraft} placeholder="请输入日志服务的 http:// 或 https:// 地址"
            onChange={event => { setDirty(true); setBaseUrlDraft(event.target.value); }} />
        </label>
        <button className="inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md bg-[#10b981] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#059669] disabled:opacity-50" type="button" disabled={!dirty} onClick={() => void saveAll()}><Save size={16} />保存</button>
        <button className="inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md border border-[#cfd8e6] bg-white px-4 py-2.5 text-sm font-medium hover:bg-[#eef3f8] disabled:opacity-50" type="button" disabled={checkingAll || dirty || !config.baseUrl || visibleEntries.length === 0} onClick={() => void testAll()} title={dirty ? "请先保存配置" : "测试所有日志连接"}><PlugZap size={16} />{checkingAll ? "测试中" : "测试连接"}</button>
      </div>

      {view !== "connection" && <div className={view === "all" ? "flex min-h-0 flex-1 flex-col gap-2.5" : "space-y-2.5"}>
        <div className="flex shrink-0 flex-wrap items-center gap-2">
          <button className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-[#cfd8e6] bg-[#f8fafc] px-3 py-2 text-xs font-medium text-[#607086] hover:bg-[#eef3f8]" type="button" onClick={() => setShowModal(true)}><Settings size={14} />日志路径配置</button>
          <div className="relative min-w-[180px] flex-1">
            <Search size={14} aria-hidden="true" className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-[#94a3b8]" />
            <input type="search" aria-label="筛选服务器配置" className="server-filter w-full rounded-md border border-[#cfd8e6] bg-white py-2 pl-8 pr-8 text-xs outline-none focus:border-[#2563eb]" placeholder="输入名称、分组、IP、路径或文件名筛选…" value={searchText} onChange={event => setSearchText(event.target.value)} onKeyDown={event => { if (event.key === "Escape") setSearchText(""); }} />
            {searchText && <button className="absolute right-2 top-1/2 -translate-y-1/2 rounded text-[#94a3b8] hover:text-[#243145]" type="button" aria-label="清空筛选" onClick={() => setSearchText("")}><X size={14} /></button>}
          </div>
          <span className="whitespace-nowrap text-xs text-[#607086]" role="status">{filtering ? `${filteredEntries.length} / ${visibleEntries.length} 项` : `共 ${visibleEntries.length} 项`}<span className="ml-3 text-[#2563eb]">已选 {selectedEntries.length}</span></span>
        </div>
        {visibleEntries.length > 0 && <div className="flex shrink-0 items-center justify-between gap-2 text-xs text-[#7b889b]">
          <span>{selectedOutsideFilter > 0 ? `筛选外仍有 ${selectedOutsideFilter} 项已勾选，操作时会一并使用` : "按名称排序 · 输入仅筛选，勾选由你决定"}</span>
          {!filtering && groups.length > 0 && <button type="button" className="shrink-0 text-[#607086] hover:text-[#2563eb]" onClick={() => setCollapsedGroups(allCollapsed ? new Set() : new Set(groups.map(([id]) => id)))}>{allCollapsed ? "全部展开" : "全部折叠"}</button>}
        </div>}
        <div className={`server-groups log-scrollbar ${view === "scope" ? "server-groups-drawer" : ""}`} aria-label="服务器配置列表">
          {groups.map(([groupId, group]) => {
            const allEnabled = group.entries.every(entry => entry.enabled);
            const count = group.entries.filter(entry => entry.enabled).length;
            const expanded = filtering || !collapsedGroups.has(groupId);
            return <div key={groupId} className="overflow-hidden rounded-md border border-[#e0e6ef]">
              <div className={`flex items-center gap-2 bg-[#f6f8fc] px-2.5 py-2 ${filtering ? "" : "cursor-pointer hover:bg-[#eef2f9]"}`} onClick={() => { if (!filtering) toggleExpand(groupId); }}>
                <button type="button" aria-label={`${expanded ? "折叠" : "展开"}${group.name}`} aria-expanded={expanded} disabled={filtering} className="rounded p-0.5 text-[#607086] hover:bg-[#e8edf5] disabled:cursor-default" onClick={event => { event.stopPropagation(); toggleExpand(groupId); }}>{expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}</button>
                <label className="inline-flex min-w-0 cursor-pointer items-center gap-2 text-xs font-semibold text-[#243145]" onClick={event => event.stopPropagation()}>
                  <input ref={element => { if (element) element.indeterminate = count > 0 && !allEnabled; }} type="checkbox" aria-label={`${filtering ? "勾选筛选结果" : "勾选分组"} ${group.name}`} className="h-3.5 w-3.5 shrink-0 accent-[#2563eb]" checked={allEnabled} onChange={() => void toggleLogEntries(group.entries.map(entry => entry.id))} />
                  <span className="truncate">{group.name}</span>
                </label>
                <span className="ml-auto shrink-0 text-xs text-[#7b889b]">已选 {count} / {group.entries.length}</span>
              </div>
              {expanded && <div className="server-entry-grid p-2">{group.entries.map(renderEntry)}</div>}
            </div>;
          })}
          {visibleEntries.length === 0 && <div className="rounded-md border border-dashed border-[#d7dfeb] px-4 py-5 text-center text-xs text-[#7b889b]">暂无服务器配置，点击“日志路径配置”添加或导入。</div>}
          {visibleEntries.length > 0 && filteredEntries.length === 0 && <div className="py-5 text-center text-xs text-[#7b889b]">没有匹配的配置，试试其他关键词或<button type="button" className="ml-1 text-[#2563eb]" onClick={() => setSearchText("")}>清空筛选</button>。</div>}
        </div>
        <p className="shrink-0 text-xs leading-5 text-[#7b889b]">勾选后可下载或查询日志；右键可打开服务器目录。.gz 文件会自动解压。</p>
      </div>}

      {contextMenu && createPortal(<div ref={menuRef} role="menu" aria-label="服务器操作" className="fixed z-[100] min-w-[144px] rounded-md border border-[#d9e1ec] bg-white py-1 shadow-lg" style={{ left: contextMenu.x, top: contextMenu.y }}>
        <button role="menuitem" className="flex w-full items-center gap-2 px-3 py-2 text-xs text-[#243145] hover:bg-[#f0f4ff]" type="button" onClick={() => { void invoke("open_file", { path: contextMenu.dirUrl }); setContextMenu(null); }}><ExternalLink size={13} />跳转</button>
      </div>, document.body)}
      {showModal && <LogPathModal onClose={() => setShowModal(false)} />}
    </section>
  );
}
