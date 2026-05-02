import { X } from "lucide-react";
import { useMemo, useState } from "react";
import type { ServerConfig } from "../types/query";

interface Props {
  server: ServerConfig | null;
  onClose: () => void;
  onSave: (server: ServerConfig) => void;
}

export function AddServerModal({ server, onClose, onSave }: Props) {
  const initial = useMemo<ServerConfig>(
    () =>
      server ?? {
        id: crypto.randomUUID(),
        name: "",
        baseUrl: "",
        enabled: true,
        displayOrder: 0,
      },
    [server],
  );
  const [draft, setDraft] = useState(initial);
  const [error, setError] = useState("");

  const validate = () => {
    try {
      const url = new URL(draft.baseUrl);
      if (url.protocol !== "http:" && url.protocol !== "https:") {
        setError("URL 必须是 http 或 https");
        return false;
      }
      setError("");
      return true;
    } catch {
      setError("URL 不合法");
      return false;
    }
  };

  return (
    <div className="fixed inset-0 z-30 flex items-center justify-center bg-[#162033]/35">
      <form
        className="w-[620px] rounded-lg border border-[#d9e0eb] bg-white shadow-xl"
        onSubmit={(event) => {
          event.preventDefault();
          if (!validate()) return;
          const url = draft.baseUrl.endsWith("/") ? draft.baseUrl : draft.baseUrl + "/";
          onSave({ ...draft, baseUrl: url });
        }}
      >
        <div className="flex items-center justify-between border-b border-[#e3e8f0] px-5 py-4">
          <h2 className="text-base font-semibold">{server ? "编辑日志 URL" : "添加日志 URL"}</h2>
          <button className="rounded-md p-1.5 text-[#5d6b82] hover:bg-[#eef3f8]" type="button" onClick={onClose} aria-label="关闭">
            <X size={18} />
          </button>
        </div>
        <div className="space-y-4 px-5 py-5">
          <label className="block text-sm font-medium">
            名称
            <input
              className="mt-1 w-full rounded-md border border-[#cfd8e6] px-3 py-2 outline-none focus:border-[#2563eb]"
              value={draft.name}
              required
              onChange={(event) => setDraft({ ...draft, name: event.target.value })}
            />
          </label>
          <label className="block text-sm font-medium">
            服务器 URL
            <input
              className="mt-1 w-full rounded-md border border-[#cfd8e6] px-3 py-2 outline-none focus:border-[#2563eb]"
              value={draft.baseUrl}
              required
              placeholder="http://.../10.142.149.124/"
              onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })}
            />
          </label>
          {error && <div className="rounded-md bg-[#fff1f3] px-3 py-2 text-sm text-[#b42318]">{error}</div>}
        </div>
        <div className="flex justify-end gap-3 border-t border-[#e3e8f0] px-5 py-4">
          <button className="rounded-md border border-[#cfd8e6] px-4 py-2 hover:bg-[#f2f5f9]" type="button" onClick={onClose}>
            取消
          </button>
          <button className="rounded-md bg-[#2563eb] px-4 py-2 font-medium text-white hover:bg-[#1d4ed8]" type="submit">
            保存
          </button>
        </div>
      </form>
    </div>
  );
}
