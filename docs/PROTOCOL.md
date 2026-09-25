# Worker JSONL Protocol

Rust Core 启动一个长驻 Python Worker。双方通过 stdin/stdout 交换 JSON Lines：每行一个完整 JSON 对象。Worker 的 stdout 只能写协议响应，诊断日志写入 stderr，由 Rust 写入 Worker 日志。Rust 为每个请求生成 UUID v7，并且只接受 response `id` 与 request `id` 完全相同的响应。

## 通用请求

```json
{"id":"018f...","method":"system.ping","params":{}}
```

## `system.ping`

成功响应：

```json
{"id":"018f...","ok":true,"result":{"version":"0.2.0"}}
```

## `pronunciation.analyze`

Rust 先用 `unicode-segmentation` 生成 token，再把 token 原样传给 Worker。`index` 从 0 连续递增，分析结果中的 `start_token` 和 `end_token` 使用半开区间 `[start_token, end_token)`。

请求：

```json
{
  "id":"018f...",
  "method":"pronunciation.analyze",
  "params":{
    "segment_id":"018f-segment",
    "text":"腧穴",
    "tokens":[
      {"index":0,"text":"腧"},
      {"index":1,"text":"穴"}
    ]
  }
}
```

成功响应：

```json
{
  "id":"018f...",
  "ok":true,
  "result":{
    "analyzer_version":"0.3.0",
    "domain_lexicon_version":"0.3.0",
    "items":[
      {
        "start_token":0,
        "end_token":2,
        "surface_text":"腧穴",
        "default_pinyin":"shu4 xue2",
        "candidate_pinyin":["shu4 xue2"],
        "risk_type":"medical_term",
        "source":"medical_lexicon_v3",
        "reason":"中医经穴术语",
        "rule_type":"acupoint",
        "confidence":"verified"
      }
    ]
  }
}
```

当前风险类型包括 `polyphone`、`rare_character`、`unknown_character`、`medical_term`、`classical_term`、`context_pronunciation`、`knowledge_conflict` 和 `textual_variant`。Worker 不返回 confirmed；Rust 把自动结果写为 `needs_review`。Worker 使用 Rust 提供的 token，不计算 Python 字符串索引。v0.3 结果可带 `warnings`，知识词典冲突使用 `ANALYZER_KNOWLEDGE_CONFLICT`，同时保留候选读音而不静默选择。`confidence` 只允许 `verified/high/medium/low`，不表示概率。

## `tts.test_connection`

Rust 只传非敏感的连接参数；凭据由 Rust 从应用数据目录下的 `tencent_credentials.json` 读取，并在启动 Worker 时注入环境变量。Worker 使用 Tencent Cloud TTS `TextToVoice` 对短文本进行实际连接检查，不保存音频。

请求：

```json
{
  "id":"018f...",
  "method":"tts.test_connection",
  "params":{"provider":"tencent","voice_type":501000,"sample_rate":16000}
}
```

成功响应：

```json
{"id":"018f...","ok":true,"result":{"provider":"tencent","request_id":"...","voice_type":501000}}
```

## `tts.synthesize`

Rust 为每次请求生成 UUID v7 `session_id` 和应用数据目录内的临时 `output_path`。Worker 不自行决定最终路径，也不访问 SQLite。

请求：

```json
{
  "id":"018f...",
  "method":"tts.synthesize",
  "params":{
    "provider":"tencent",
    "text":"腧穴。",
    "tokens":[{"index":0,"text":"腧"},{"index":1,"text":"穴"},{"index":2,"text":"。"}],
    "pronunciations":[{"start_token":0,"end_token":2,"surface_text":"腧穴","pinyin":"shu4 xue2"}],
    "voice_type":501000,
    "sample_rate":16000,
    "codec":"wav",
    "speed":0,
    "volume":0,
    "session_id":"018f-session",
    "output_path":"/app-data/projects/book/audio/segment/.tmp-018f.wav"
  }
}
```

`pronunciations` 为空时，Worker 把原文直接传给供应商，不生成 SSML。存在已确认覆盖时，Worker 先校验 token 范围、surface_text、拼音格式和拼音音节数量，再生成 XML 转义后的 `<speak><phoneme alphabet="py" ph="...">...</phoneme></speak>`。

成功响应：

```json
{
  "id":"018f...",
  "ok":true,
  "result":{
    "provider":"tencent",
    "request_id":"...",
    "session_id":"...",
    "output_path":"/app-data/projects/book/audio/segment/.tmp-018f.wav",
    "byte_length":12345,
    "ssml_used":true,
    "ssml":"<speak>...</speak>",
    "duration_ms":null
  }
}
```

Rust 验证 `output_path`、RIFF/WAVE 文件头和事务写入；TTS 响应超时为 60 秒，连接测试超时为 30 秒，超时会映射为 `TTS_TIMEOUT` 并结束当前 Worker 进程。

## 统一错误响应

```json
{"id":"018f...","ok":false,"error":{"code":"METHOD_NOT_FOUND","message":"unknown method: example"}}
```

Worker 当前可能返回 `INVALID_JSON`、`INVALID_REQUEST`、`METHOD_NOT_FOUND`、分析失败或 TTS 错误。TTS 错误包括 `TTS_CREDENTIALS_MISSING`、`TTS_AUTH_ERROR`、`TTS_SERVICE_NOT_ENABLED`、`TTS_RATE_LIMITED`、`TTS_QUOTA_EXHAUSTED`、`TTS_TEXT_TOO_LONG`、`TTS_INVALID_SSML` 和 `TTS_PROVIDER_ERROR`。Rust 还会把超时、Worker 退出、非法 JSON、response ID 不匹配和分析结果校验失败转换为应用错误，例如 `TTS_TIMEOUT`、`WORKER_TIMEOUT`、`WORKER_EXITED`、`WORKER_INVALID_RESPONSE`、`INVALID_ANALYSIS_RESPONSE`、`PINYIN_INVALID` 和 `PINYIN_TOKEN_COUNT_MISMATCH`。

## 拼音格式

内部格式为小写 ASCII 字母加数字声调，例如 `shu4`、`xue2`、`nv3`、`lv4`。Rust 会 trim 并规范化多余空白，然后拒绝无声调、带重音/ü、含大写、声调 6 或音节数量与汉字 token 数量不一致的输入。
