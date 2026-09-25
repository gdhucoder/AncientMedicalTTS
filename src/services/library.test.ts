import { describe, expect, it } from "vitest";
import { ttsSettingsCommandPayload } from "./library";
import type { TtsSettings } from "../types/library";

describe("TTS settings IPC payload", () => {
  it("maps Rust command arguments to Tauri camelCase keys", () => {
    const settings: TtsSettings = {
      provider: "tencent",
      voice_type: 501000,
      speed: 0,
      volume: 0,
      sample_rate: 16000,
      codec: "wav",
    };

    expect(ttsSettingsCommandPayload(settings)).toEqual({
      provider: "tencent",
      voiceType: 501000,
      speed: 0,
      volume: 0,
      sampleRate: 16000,
    });
  });
});
