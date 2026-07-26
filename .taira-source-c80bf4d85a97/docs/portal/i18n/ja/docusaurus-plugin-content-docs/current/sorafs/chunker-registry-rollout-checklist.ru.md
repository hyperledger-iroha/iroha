---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリロールアウトチェックリスト
タイトル: Чеклист ロールアウト рестра chunker SoraFS
Sidebar_label: ロールアウト チャンカーのロールアウト
説明: ロールアウト チャンカーのロールアウト。
---

:::note Канонический источник
`docs/source/sorafs/chunker_registry_rollout_checklist.md` です。 Держите обе копии синхронизированными, пока набор документации スフィンクス не будет выведен из эксплуатации.
:::

# ロールアウト SoraFS

Этот чеклист фиксирует заги, необходимые для продвижения нового профиля chunker
バンドル プロバイダーの入場料を支払うには、次の手順を実行します。
ガバナンス憲章。

> **Область:** применяется ко всем релизам, которые меняют
> `sorafs_manifest::chunker_registry`、プロバイダー入場エンベロープ или
> フィクスチャ バンドル (`fixtures/sorafs_chunker/*`)。

## 1. Предварительная валидация

1. フィクスチャとその日のスケジュール:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Убедитесь、что ハッシュ детерминизма в
   `docs/source/sorafs/reports/sf1_determinism.md` ( или релевантном отчете
   профиля) совпадают с регенерированными артефактами。
3. Убедитесь、что `sorafs_manifest::chunker_registry` компилируется с
   `ensure_charter_compliance()` の結果:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. 文書の内容:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` 分の所要時間
   - Отчет о детерминизме

## 2. ガバナンスの承認

1. ツールワーキンググループのダイジェスト版
   ソラ議会インフラパネル。
2. Зафиксируйте детали одобрения в
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`。
3. 封筒、備品、備品:
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. ガバナンスを支援するエンベロープ:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. ステージングのロールアウト

Подробный ウォークスルー см。 в [ステージング マニフェスト プレイブック](./staging-manifest-playbook)。

1. Torii と検出 `torii.sorafs` および施行
   入場 (`enforce_admission = true`)。
2. 承認されたプロバイダーの入場エンベロープとステージング レジストリ ディレクトリを作成します。
   `torii.sorafs.discovery.admission.envelopes_dir` です。
3. プロバイダー広告のディスカバリー API:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. エンドポイントのマニフェスト/プランとガバナンス ヘッダーを設定します。
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Убедитесь、что テレメトリ ダッシュボード (`torii_sorafs_*`) およびアラート ルール
   жотображают новый профиль без осибок.

## 4. 本番環境への展開

1. ステージング Torii-узлах を参照してください。
2. Объявите окно активации (дата/время、猶予期間、ロールバック計画) と каналы
   SDK と互換性があります。
3. Смёрджите релизный PR с:
   - 備品と封筒
   - Документационными изменениями (ссылки на charter、отчет о детерминизме)
   - ロードマップ/ステータス
4. 起源を知ることができます。

## 5. ロールアウト後の作業

1. Снимите финальные метрики (検出数、フェッチ成功率、エラー)
   ヒストグラム) 24 時間ロールアウト。
2. Обновите `status.md` кратким резюме и ссылкой на отчет детерминизма.
3. フォローアップ (ナビゲーション、オーサリング、ガイダンス)
   ) в `roadmap.md`。