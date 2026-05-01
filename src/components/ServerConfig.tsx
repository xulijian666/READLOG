import { CheckCircle2, Edit2, PlugZap, Plus, Save, Trash2, XCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "../lib/runtime";
import { useServerStore } from "../store/serverStore";
import type { ConnectionCheckResult, ServerConfig as Server } from "../types/query";
import { AddServerModal } from "./AddServerModal";

type CheckState = Record<string, { loading: boolean; result?: ConnectionCheckResult; error?: string }>;

const LOG_TYPES = ["app", "sql", "dlp", "monitor"];

export function ServerConfig() {
  const { config, toggleServer, upsertServer, deleteServer, save } = useServerStore();
  const [editing, setEditing] = useState<Server | null>(null);
  const [adding, setAdding] = useState(false);
  const [authDraft, setAuthDraft] = useState({ username: "", password: "" });
  const [authDirty, setAuthDirty] = useState(false);
  const [checkingAll, setCheckingAll] = useState(false);
  const [checks, setChecks] = useState<CheckState>({});
  const [logType, setLogType] = useState("app");
  const [logTypeDirty, setLogTypeDirty] = useState(false);

  useEffect(() => {
    if (!config || authDirty) return;
    setAuthDraft(config.credentials);
  }, [authDirty, config]);

  useEffect(() => {
    if (!config || logTypeDirty) return;
    setLogType(config.settings.logType || "app");
  }, [logTypeDirty, config]);

  if (!config) return null;

  const saveCredentials = async () => {
    await save({ ...config, credentials: authDraft });
    setAuthDirty(false);
  };

  const saveLogType = async () => {
    await save({ ...config, settings: { ...config.settings, logType } });
    setLogTypeDirty(false);
  };

  const testAll = async () => {
    setCheckingAll(true);
    setChecks(
      Object.fromEntries(config.servers.map((server) => [server.id, { loading: true }])),
    );
    try {
      const results = await invoke<ConnectionCheckResult[]>("test_all_connections", {
        serverIds: config.servers.map((server) => server.id),
      });
      setChecks(
        Object.fromEntries(results.map((result) => [result.serverId, { loading: false, result }])),
      );
    } catch (error) {
      setChecks(
        Object.fromEntries(config.servers.map((server) => [server.id, { loading: false, error: String(error) }])),
      );
    } finally {
      setCheckingAll(false);
    }
  };

  return (
    <section className="border-b border-[#d9e1ec] bg-white px-5 py-4">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-sm font-semibold text-[#243145]">日志 URL 与认证</h2>
        <button
          className="inline-flex items-center gap-2 rounded-md bg-[#2563eb] px-3 py-2 text-sm font-medium text-white hover:bg-[#1d4ed8]"
          onClick={() => setAdding(true)}
          type="button"
        >
          <Plus size={16} /> 添加 URL
        </button>
      </div>

      <div
        className="mb-4 grid max-w-[980px] items-end gap-4 rounded-lg border border-[#d7dfeb] bg-[#f8fafc] p-4"
        style={{ gridTemplateColumns: "1fr 1fr auto auto auto" }}
      >
        <label className="block text-sm font-medium">
          统一用户名
          <input
            className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
            value={authDraft.username}
            onChange={(event) => {
              setAuthDirty(true);
              setAuthDraft((draft) => ({ ...draft, username: event.target.value }));
            }}
          />
        </label>
        <label className="block text-sm font-medium">
          统一密码
          <input
            className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
            type="password"
            value={authDraft.password}
            onChange={(event) => {
              setAuthDirty(true);
              setAuthDraft((draft) => ({ ...draft, password: event.target.value }));
            }}
          />
        </label>
        <label className="block text-sm font-medium">
          日志类型
          <select
            className="mt-1 w-full rounded-md border border-[#cfd8e6] bg-white px-3 py-2 outline-none focus:border-[#2563eb]"
            value={logType}
            onChange={(event) => {
              setLogTypeDirty(true);
              setLogType(event.target.value);
            }}
          >
            {LOG_TYPES.map((type) => (
              <option key={type} value={type}>
                {type}.log
              </option>
            ))}
          </select>
        </label>
        <button
          className="inline-flex items-center gap-2 rounded-md bg-[#10b981] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#059669] disabled:opacity-50"
          type="button"
          disabled={!authDirty && !logTypeDirty}
          onClick={() => {
            void saveCredentials();
            void saveLogType();
          }}
        >
          <Save size={16} /> 保存
        </button>
        <button
          className="inline-flex items-center gap-2 rounded-md border border-[#cfd8e6] bg-white px-4 py-2.5 text-sm font-medium hover:bg-[#eef3f8] disabled:opacity-50"
          type="button"
          disabled={checkingAll || authDirty || logTypeDirty || config.servers.length === 0}
          onClick={() => void testAll()}
          title={authDirty || logTypeDirty ? "请先保存配置" : "测试所有 URL"}
        >
          <PlugZap size={16} /> {checkingAll ? "测试中" : "测试连接"}
        </button>
      </div>

      <div className="space-y-2">
        {config.servers.map((server, index) => {
          const check = checks[server.id];
          return (
            <div key={server.id} className="flex flex-wrap items-center gap-2 rounded-md border border-[#d7dfeb] bg-[#f8fafc] px-3 py-2">
              <input
                className="h-4 w-4 accent-[#2563eb]"
                type="checkbox"
                checked={server.enabled}
                onChange={() => void toggleServer(server.id)}
                aria-label={`${server.name} 勾选下载`}
              />
              <span className={`h-2.5 w-2.5 rounded-full ${["bg-[#2563eb]", "bg-[#8b5cf6]", "bg-[#f97316]"][index % 3]}`} />
              <span className="min-w-[86px] text-sm font-medium">{server.name}</span>
              <span className="max-w-[680px] truncate text-xs text-[#69778c]">{server.baseUrl}</span>
              <button className="rounded p-1 text-[#607086] hover:bg-[#e8edf5]" type="button" onClick={() => setEditing(server)} aria-label="编辑">
                <Edit2 size={15} />
              </button>
              <button className="rounded p-1 text-[#b42318] hover:bg-[#fee4e2]" type="button" onClick={() => void deleteServer(server.id)} aria-label="删除">
                <Trash2 size={15} />
              </button>
              {check?.loading && <span className="ml-auto text-xs text-[#607086]">连接中</span>}
              {check?.result && (
                <span className={`ml-auto inline-flex items-center gap-1 text-xs ${check.result.ok ? "text-[#047857]" : "text-[#b42318]"}`}>
                  {check.result.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
                  {check.result.message}
                </span>
              )}
              {check?.error && (
                <span className="ml-auto inline-flex items-center gap-1 text-xs text-[#b42318]">
                  <XCircle size={14} />
                  {check.error}
                </span>
              )}
            </div>
          );
        })}
      </div>

      <p className="mt-3 text-xs text-[#69778c]">
        配置服务器 URL（例如 http://10.142.149.25:61000/.../10.142.149.124/），测试连接和下载时会自动拼接所选日志类型（{logType}.log）。
      </p>
      {(editing || adding) && (
        <AddServerModal
          server={editing}
          onClose={() => {
            setEditing(null);
            setAdding(false);
          }}
          onSave={(server) => {
            void upsertServer(server);
            setEditing(null);
            setAdding(false);
          }}
        />
      )}
    </section>
  );
}
