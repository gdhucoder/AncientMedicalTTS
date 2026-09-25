import type { ExportState } from "../types/library";

export function sanitizeExportFilename(title: string, format: "mp3" | "wav"): string {
  const safeTitle = title.replace(/[<>:"/\\|?*]/g, "_").replace(/[. _]+$/g, "").trim() || "ancient-medical-tts";
  return `${safeTitle}.${format}`;
}

export function exportPhaseLabel(state: ExportState): string {
  const labels: Record<string, string> = {
    preparing: "准备音频…",
    merging: "正在合并语音片段…",
    encoding_mp3: "正在生成 MP3…",
    saving: "正在保存文件…",
    cancelling: "正在取消…",
  };
  return labels[state.phase] ?? "正在导出完整音频…";
}
