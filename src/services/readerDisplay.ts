const TONE_MARKS: Record<string, readonly string[]> = {
  a: ["ā", "á", "ǎ", "à", "a"],
  e: ["ē", "é", "ě", "è", "e"],
  i: ["ī", "í", "ǐ", "ì", "i"],
  o: ["ō", "ó", "ǒ", "ò", "o"],
  u: ["ū", "ú", "ǔ", "ù", "u"],
  ü: ["ǖ", "ǘ", "ǚ", "ǜ", "ü"],
};

function formatSyllable(syllable: string): string {
  const match = /^([a-züv:]+)([1-5])$/.exec(syllable.toLowerCase());
  if (!match) return syllable;
  const letters = match[1].replace(/v|u:/g, "ü");
  const tone = Number(match[2]);
  if (tone === 5) return letters;

  let vowelIndex = letters.indexOf("a");
  if (vowelIndex < 0) vowelIndex = letters.indexOf("e");
  if (vowelIndex < 0) {
    const ouIndex = letters.indexOf("ou");
    vowelIndex = ouIndex >= 0 ? ouIndex : -1;
  }
  if (vowelIndex < 0) {
    for (let index = letters.length - 1; index >= 0; index -= 1) {
      if ("aeiouü".includes(letters[index])) {
        vowelIndex = index;
        break;
      }
    }
  }
  if (vowelIndex < 0) return letters;
  const vowel = letters[vowelIndex];
  const marks = TONE_MARKS[vowel];
  if (!marks) return syllable;
  return `${letters.slice(0, vowelIndex)}${marks[tone - 1]}${letters.slice(vowelIndex + 1)}`;
}

export function pinyinToToneMarks(value: string | null | undefined): string {
  if (!value?.trim()) return "";
  return value.trim().split(/\s+/).map(formatSyllable).join(" ");
}
