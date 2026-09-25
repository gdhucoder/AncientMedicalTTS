export function friendlyErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (isErrorPayload(error)) return error.message;
  return "操作失败，请查看日志。";
}

function isErrorPayload(error: unknown): error is { code: string; message: string } {
  if (typeof error !== "object" || error === null) return false;
  const value = error as Record<string, unknown>;
  return typeof value.code === "string" && typeof value.message === "string";
}
