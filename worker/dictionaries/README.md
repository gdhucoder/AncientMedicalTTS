# Worker pronunciation dictionaries

这些 JSON 是随 Worker 发布的程序资源，不是用户数据，也不读取评测 Gold。

- `medical_lexicon_v3.json`：结构化中医词典，按病证、症状、脉象、经络、脏腑、方剂、药物、腧穴、治法等类别维护。
- `classical_lexicon_v3.json`：独立的古籍、生僻字词和古义词资源。
- `context_rules_v3.json`：以 focus、左右/邻近词、窗口和优先级描述的 semantic context 规则。
- `context_pronunciation_rules.json`：保留的确定性完整短语读音规则；只支持 exact match。
- `high_risk_polyphones.json`：有限高风险字清单，用于降低普通 pypinyin 多音字噪声。
- `known_rare_characters.json`：已知生僻或古籍字，即使有 pypinyin 读音也提示人工复核。
- `variant_characters.json`：只在分析阶段建立 surface/canonical 关系，绝不改写原始文本。

v3 词条必须包含非空来源和 `verified/high/medium/low` 置信级别；拼音仍为 ASCII tone-number，并且音节数必须与可发音汉字数一致。新增或修改资源后运行 `uv run --project worker python tools/validate_pronunciation_lexicon.py`。数据以通用中医术语、古籍词语和可解释的语境读法为依据维护，禁止把 benchmark Gold 当作来源。用户在应用中建立的 Book/Global Pronunciation Rule 存在 SQLite 的独立 `pronunciation_rules` 表，不应把两类数据混为一谈。
