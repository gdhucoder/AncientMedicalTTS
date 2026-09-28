import { isTauri } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";

const UPDATE_CHECK_TIMEOUT_MS = 15_000;

export async function checkForAppUpdate(): Promise<Update | null> {
  if (!isTauri()) return null;
  return check({ timeout: UPDATE_CHECK_TIMEOUT_MS });
}

