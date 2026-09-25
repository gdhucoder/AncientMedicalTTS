import { describe, expect, it } from "vitest";
import { exportPhaseLabel, sanitizeExportFilename } from "./exportUi";

describe("export UI helpers", () => {
  it("sanitizes cross-platform filename characters and suffixes", () => {
    expect(sanitizeExportFilename("黄帝:内经? ", "mp3")).toBe("黄帝_内经.mp3");
    expect(sanitizeExportFilename("...", "wav")).toBe("ancient-medical-tts.wav");
  });

  it("maps export phases to concise user-facing text", () => {
    expect(exportPhaseLabel({ phase: "merging" } as never)).toBe("正在合并语音片段…");
    expect(exportPhaseLabel({ phase: "unknown" } as never)).toBe("正在导出完整音频…");
  });
});
