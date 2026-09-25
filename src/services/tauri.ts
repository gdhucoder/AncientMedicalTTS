import { invoke } from "@tauri-apps/api/core";
import type { DeveloperStatus, WorkerPing } from "../types/status";

export function getStatus(): Promise<DeveloperStatus> {
  return invoke<DeveloperStatus>("get_status");
}

export function pingWorker(): Promise<WorkerPing> {
  return invoke<WorkerPing>("ping_worker");
}
