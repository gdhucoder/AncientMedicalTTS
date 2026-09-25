import { create } from "zustand";
import { getStatus, pingWorker } from "../services/tauri";
import { friendlyErrorMessage } from "../services/errors";
import type { DeveloperStatus, WorkerPing } from "../types/status";

type StatusState = {
  status: DeveloperStatus | null;
  lastPing: WorkerPing | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  ping: () => Promise<void>;
};

export const useStatusStore = create<StatusState>((set) => ({
  status: null,
  lastPing: null,
  loading: true,
  error: null,
  refresh: async () => {
    set({ loading: true, error: null });
    try {
      set({ status: await getStatus(), loading: false });
    } catch (error: unknown) {
      set({ loading: false, error: friendlyErrorMessage(error) });
    }
  },
  ping: async () => {
    set({ loading: true, error: null });
    try {
      const result = await pingWorker();
      set({ lastPing: result, status: await getStatus(), loading: false });
    } catch (error: unknown) {
      set({ loading: false, error: friendlyErrorMessage(error) });
    }
  },
}));
