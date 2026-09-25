import { beforeEach, describe, expect, it, vi } from "vitest";
import { getStatus, pingWorker } from "../services/tauri";
import { useStatusStore } from "./statusStore";
import type { DeveloperStatus, WorkerPing } from "../types/status";

vi.mock("../services/tauri", () => ({
  getStatus: vi.fn(),
  pingWorker: vi.fn(),
}));

const mockedGetStatus = vi.mocked(getStatus);
const mockedPingWorker = vi.mocked(pingWorker);

const status: DeveloperStatus = {
  application: { ok: true, message: "应用正在运行" },
  database: { ok: true, message: "SQLite 已连接" },
  worker: { ok: true, running: true, version: "0.1.0", message: "运行中" },
};

describe("status store", () => {
  beforeEach(() => {
    mockedGetStatus.mockReset();
    mockedPingWorker.mockReset();
    useStatusStore.setState({ status: null, lastPing: null, loading: true, error: null });
  });

  it("loads the developer status", async () => {
    mockedGetStatus.mockResolvedValue(status);

    await useStatusStore.getState().refresh();

    expect(useStatusStore.getState().status).toEqual(status);
    expect(useStatusStore.getState().error).toBeNull();
  });

  it("stores the ping result and refreshes status", async () => {
    const ping: WorkerPing = { version: "0.1.0", elapsed_ms: 3 };
    mockedPingWorker.mockResolvedValue(ping);
    mockedGetStatus.mockResolvedValue(status);

    await useStatusStore.getState().ping();

    expect(useStatusStore.getState().lastPing).toEqual(ping);
    expect(useStatusStore.getState().status).toEqual(status);
  });
});
