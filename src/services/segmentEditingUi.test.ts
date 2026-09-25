import { describe, expect, it } from "vitest";
import { graphemeIndexAtCaret, graphemeTokenCount } from "./segmentEditingUi";

describe("segment editing caret mapping", () => {
  it("maps a Unicode caret to a grapheme boundary", () => {
    const text = "A𠮷B";
    expect(graphemeTokenCount(text)).toBe(3);
    expect(graphemeIndexAtCaret(text, text.indexOf("B"))).toBe(2);
  });

  it("does not split inside a supplementary-plane grapheme", () => {
    const text = "甲𠮷乙";
    expect(graphemeIndexAtCaret(text, 2)).toBe(1);
    expect(graphemeIndexAtCaret(text, 3)).toBe(2);
  });
});
