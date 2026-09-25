import { describe, expect, it } from "vitest";
import { friendlyErrorMessage } from "./errors";

describe("friendlyErrorMessage", () => {
  it("shows the Rust error message", () => {
    expect(friendlyErrorMessage({ code: "EMPTY_TEXT", message: "文本内容为空" })).toBe("文本内容为空");
  });

  it("falls back for unknown errors", () => {
    expect(friendlyErrorMessage({ unexpected: true })).toBe("操作失败，请查看日志。");
  });
});
