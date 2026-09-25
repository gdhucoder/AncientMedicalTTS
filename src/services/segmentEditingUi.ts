/**
 * Converts a textarea caret offset (UTF-16 code units) to the corresponding
 * Unicode grapheme-token boundary. The Rust side remains authoritative and
 * validates the resulting token index before changing the database.
 */
export function graphemeIndexAtCaret(text: string, caretOffset: number): number {
  const caret = Math.max(0, Math.min(caretOffset, text.length));
  const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });
  let index = 0;
  for (const part of segmenter.segment(text)) {
    const end = part.index + part.segment.length;
    if (caret <= part.index || caret < end) return index;
    index += 1;
  }
  return index;
}

export function graphemeTokenCount(text: string): number {
  return Array.from(new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(text)).length;
}
