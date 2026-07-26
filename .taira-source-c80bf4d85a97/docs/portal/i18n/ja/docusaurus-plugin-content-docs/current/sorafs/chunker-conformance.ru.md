---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: チャンカー準拠
タイトル: Руководство по соответствию チャンカー SoraFS
サイドバーラベル: チャンカー
説明: チャンカー SF1、フィクスチャ、SDK のワークフローとワークフロー。
---

:::note Канонический источник
:::

Этот документ фиксирует требования, которым должна следовать каждая реализация, чтобы
チャンカー SoraFS (SF1) が見つかりました。 Он также
документирует ワークフロー регенерации、политику、подписей и ги верификации、чтобы
フィクスチャと SDK を組み合わせて使用できます。

## Канонический профиль

- Входной シード (16 進数): `0000000000dec0ded`
- バイト数: 262144 バイト (256 KiB)
- Минимальный размер: 65536 バイト (64 KiB)
- メッセージ: 524288 バイト (512 KiB)
- ローリング多項式: `0x3DA3358B4DC173`
- シードギア: `sorafs-v1-gear`
- ブレークマスク：`0x0000FFFF`

バージョン: `sorafs_chunker::chunk_bytes_with_digests_profile`。
SIMD は、ダイジェストを表示します。

## Набор 器具

`cargo run --locked -p sorafs_chunker --bin export_vectors` регенерирует
備品と`fixtures/sorafs_chunker/`の情報:

- `sf1_profile_v1.{json,rs,ts,go}` — канонические границы чанков для
  Rust、TypeScript、Go に対応します。 Каждый файл объявляет канонический ハンドル как
  `sorafs.sf1@1.0.0`、`sorafs.sf1@1.0.0`)。 Порядок фиксируется
  `ensure_charter_compliance` と НЕ ДОЛЖЕН изменяться。
- `manifest_blake3.json` — BLAKE3-верифициров​​анный マニフェスト、покрывающий каждый файл フィクスチャ。
- `manifest_signatures.json` — ダイジェスト版 (Ed25519) です。
- `sf1_profile_v1_backpressure.json` と `fuzz/` のコーパス —
  バック プレッシャー チャンカーを使用してストリーミングを行うことができます。

### Политика подписей

試合の試合 **должна** が表示されます。 Генератор
отклоняет неподписанный вывод, если явно не передан `--allow-unsigned` (предназначен)
только для локальных экспериментов)。追加専用です。
дедуплицируются по подписанту。

Чтобы добавить подпись совета:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Верификация

CI ヘルパー `ci/check_sorafs_fixtures.sh` の機能
`--locked`。試合の試合、仕事の記録。 Используйте
夜間のワークフローとフィクスチャを備えています。

概要:

1. `cargo test -p sorafs_chunker` です。
2. Выполните `ci/check_sorafs_fixtures.sh` локально。
3. Убедитесь、что `git status -- fixtures/sorafs_chunker` чист。

## Плейбук обновления

SF1 のチャンカー:

См. также: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) 日付
требований к метаданным, заблонов предложений и чеклистов валидации.

1. `ChunkProfileUpgradeProposalV1` (RFC SF-1) を参照してください。
2. フィクスチャ через `export_vectors` および новый ダイジェスト メソッド。
3. Подпизите манифест требуемым кворумом совета. Все подписи должны быть
   `manifest_signatures.json` です。
4. SDK フィクスチャ (Rust/Go/TS) とランタイムをサポートします。
5. ファズコーパスを参照してください。
6. 処理、シード、ダイジェストを実行します。
7. ロードマップを確認してください。

Изменения、которые затрагивают границы чанков или ダイジェスト без соблюдения этого процесса,
あなたのことを考えてください。