import { create } from "zustand";
import { invoke } from "../lib/runtime";
import type { AppConfig, ServerConfig } from "../types/query";

interface ServerState {
  config: AppConfig | null;
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  save: (config: AppConfig) => Promise<void>;
  upsertServer: (server: ServerConfig) => Promise<void>;
  deleteServer: (serverId: string) => Promise<void>;
  toggleServer: (serverId: string) => Promise<void>;
}

export const useServerStore = create<ServerState>((set, get) => ({
  config: null,
  loading: false,
  error: null,
  load: async () => {
    set({ loading: true, error: null });
    try {
      const config = await invoke<AppConfig>("load_app_config");
      set({ config, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },
  save: async (config) => {
    set({ loading: true, error: null });
    try {
      const saved = await invoke<AppConfig>("save_app_config", { config });
      set({ config: saved, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },
  upsertServer: async (server) => {
    const config = get().config;
    if (!config) return;
    const exists = config.servers.some((item) => item.id === server.id);
    const servers = exists
      ? config.servers.map((item) => (item.id === server.id ? server : item))
      : [...config.servers, { ...server, displayOrder: config.servers.length }];
    await get().save({ ...config, servers });
  },
  deleteServer: async (serverId) => {
    const config = get().config;
    if (!config) return;
    const servers = config.servers
      .filter((server) => server.id !== serverId)
      .map((server, displayOrder) => ({ ...server, displayOrder }));
    await get().save({ ...config, servers });
  },
  toggleServer: async (serverId) => {
    const config = get().config;
    if (!config) return;
    const servers = config.servers.map((server) =>
      server.id === serverId ? { ...server, enabled: !server.enabled } : server,
    );
    await get().save({ ...config, servers });
  },
}));
