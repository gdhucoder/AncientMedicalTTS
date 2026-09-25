import { describe, expect, it } from "vitest";
import { pinyinToToneMarks } from "./readerDisplay";

describe("pinyinToToneMarks", () => {
  it("converts tone-number pinyin to tone marks", () => {
    expect(pinyinToToneMarks("shu4 xue2")).toBe("shù xué");
    expect(pinyinToToneMarks("lv4 nve4")).toBe("lǜ nüè");
  });

  it("leaves neutral and invalid values safe for display", () => {
    expect(pinyinToToneMarks("de5")).toBe("de");
    expect(pinyinToToneMarks("not-pinyin")).toBe("not-pinyin");
    expect(pinyinToToneMarks(null)).toBe("");
  });
});
