---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: オーケストレーター構成
タイトル: Конфигурация оркестратора SoraFS
サイドバーラベル: Конфигурация оркестратора
説明: Настройка мульти-источникового fetch-оркестратора、интерпретация сбоев и отладка телеметрии.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/developer/orchestrator.md`。 Собе копии синхронизированными, пока устаревДая документация не будет выведена из обращения.
:::

# Руководство по мульти-источниковому fetch-оркестратору

Мульти-источниковый fetch-оркестратор SoraFS управляет детерминированными
広告を表示するだけでなく、広告も表示されます。
統治。 В этом руководстве описано, как настраивать оркестратор,
ロールアウトを実行したり、ロールアウトを実行したりすることができます。
индикаторы здоровья。

## 1. Обзор конфигурации

Оркестратор объединяет три источника конфигурации:

| Источник | Назначение | Примечания |
|----------|-----------|---------------|
| `OrchestratorConfig.scoreboard` | JSON スコアボードのスコアボードを表示します。 | Основан на `crates/sorafs_car::scoreboard::ScoreboardConfig`。 |
| `OrchestratorConfig.fetch` |ランタイム - ограничения (бюджеты ретраев、лимиты параллелизма、переключатели верификации)。 | `FetchOptions` と `crates/sorafs_car::multi_fetch` を確認してください。 |
| CLI / SDK を使用する |拒否/ブーストを実行します。 | `sorafs_cli fetch` раскрывает эти флаги напрямую; SDK は `OrchestratorConfig` に対応しています。 |

JSON-хелперы と `crates/sorafs_orchestrator::bindings` を確認してください
Norito JSON と、SDK および автоматизацией を参照してください。

### 1.1 JSON-конфигурации

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Сохраняйте файл через стандартное наслаивание `iroha_config` (`defaults/`、ユーザー、
実際)、чтобы детерминированные деплои наследовали одинаковые лимиты на всех
ね。直接のみ、ロールアウト SNNet-5a を使用してください。
`docs/examples/sorafs_direct_mode_policy.json` と表示されます。
`docs/source/sorafs/direct_mode_pack.md`。

### 1.2 オーバーライド

SNNet-9 は、ガバナンス主導のコンプライアンスを実現します。 Новый объект
`compliance` と конфигурации Norito JSON のカーブアウト、構造
パイプラインフェッチ × 直接のみ:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` объявляет коды ISO-3166 alpha-2, гдеработает эта
  инстанция оркестратора。 Коды нормализуются в верхний регистр при парсинге.
- `jurisdiction_opt_outs` ガバナンス。 Когда любая юрисдикция
  Аператора присутствует в списке, оркестратор применяет
  `transport_policy=direct-only` またはフォールバック
  `compliance_jurisdiction_opt_out`。
- `blinded_cid_opt_outs` ダイジェスト メッセージ (ブラインド CID、ビュー
  16 進数)。ペイロードを直接のみ表示する
  フォールバック `compliance_blinded_cid_opt_out` から。
- `audit_contacts` URI のガバナンスと GAR の接続
  プレイブックが見つかります。
- `attestations` фиксирует подписанные コンプライアンス - пакеты、на которых держится
  。 `jurisdiction` (ISO‑3166 alpha‑2)、
  `document_uri`、канонический `digest_hex` (64 символа)、タイムスタンプ
  `issued_at_ms` または опциональный `expires_at_ms`。 Эти артефакты попадают в
  аудит-чеклист оркестратора, чтобы ガバナンス ツールをオーバーライドする
  そうです。

Передавайте блок コンプライアンス через стандартное наслаивание конфигурации, чтобы
オーバーライドします。 Оркестратор применяет
コンプライアンス _после_ 書き込みモードのヒント: SDK の `upload-pq-only`、
オプトアウトは、直接のみを許可するものではありません。
и быстро завергерой, если не осталось совместимых провайдеров.

オプトアウトを選択する
`governance/compliance/soranet_opt_outs.json`;ガバナンスを強化する
обновления через タグ付きリリース。 Полный пример конфигурации (включая)
証明書) доступен в `docs/examples/sorafs_compliance_policy.json`, а
операционный процесс описан в
[プレイブック соответствия GAR](../../../source/soranet/gar_compliance_playbook.md)。

### 1.3 CLI と SDK の説明| Флаг / Поле | Эффект |
|---------------|----------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Ограничивает、сколько провайдеров пройдут фильтр スコアボード。 `None`、適格な条件を満たしています。 |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Ограничивает число ретраев на チャンク。 `MultiSourceError::ExhaustedRetries` を参照してください。 |
| `--telemetry-json` |スナップショットとスコアボードのスナップショット。 `telemetry_grace_secs` が対象となります。 |
| `--scoreboard-out` | Сохраняет вычисленный スコアボード (適格 + 不適格のスコア) が表示されます。 |
| `--scoreboard-now` |タイムスタンプ スコアボード (Unix 秒)、キャプチャ フィクスチャを確認できます。 |
| `--deny-provider` / フック スコア |広告を表示します。ブラックリストに登録されています。 |
| `--boost-provider=name:delta` |加重ラウンドロビンによるガバナンスを実現します。 |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` |ロールアウトを実行して、ロールアウトを実行します。 |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | По умолчанию `soranet-first`、так как мульти-источниковый оркестратор — базовый. `direct-only` はダウングレードし、コンプライアンスを遵守し、`soranet-strict` は PQ のみをサポートします。コンプライアンスは、остаются жестким потолком を上書きします。 |

SoraNet ファーストのテスト、ロールバックのテストが必要です。
SNNet ブロッカー。 SNNet-4/5/5a/5b/6a/7/8/12/13 のガバナンスを確認する
требуемую позу (в сторону `soranet-strict`); до этого только オーバーライド по
инцидентам должны приоритизировать `direct-only`, их нужно фиксировать в log
ロールアウト。

Все флаги выге принимают синтаксис `--` как в `sorafs_cli fetch`, так и в
`sorafs_fetch` です。 SDK の型指定
建設業者。

### 1.4 ガード キャッシュの説明

CLI は、SoraNet、セレクター ガードをサポートします。
SNNet-5 のロールアウトとエントリ リレーの接続。
Рабочий процесс контролируют три новых флага:

| Флаг | Назначение |
|------|-----------|
| `--guard-directory <PATH>` | JSON-файл с последним リレーコンセンサス (подмножество ниже) を使用します。ディレクトリを保護し、キャッシュをガードしてフェッチします。 |
| `--guard-cache <PATH>` | Norito でエンコードされた `GuardSet`。キャッシュ、ディレクトリのディレクトリを参照してください。 |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Опциональные は、для числа エントリ ガード (по умолчанию 3) および окна удержания (по умолчанию 30 dayней) をオーバーライドします。 |
| `--guard-cache-key <HEX>` | Опциональный 32-байтовый ключ для тега ガード キャッシュ с Blake3 MAC、чтобы файл можно было проверить перед повторнымそうです。 |

ペイロード ガード ディレクトリの説明:

`--guard-directory` を使用する Norito でエンコードされたペイロード
`GuardDirectorySnapshotV2`。スナップショットの例:

- `version` — версия схемы (сейчас `2`)。
- `directory_hash`、`published_at_unix`、`valid_after_unix`、`valid_until_unix` —
  コンセンサスが得られれば、合意が得られます。
- `validation_phase` — ゲート политики сертификатов (`1` = разрезить одну Ed25519)
  подпись, `2` = предпочесть двойные подписи, `3` = требовать двойные подписи)。
- `issuers` — ガバナンス с `fingerprint`、`ed25519_public` および
  `mldsa65_public`。指紋認証
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`。
- `relays` — SRCv2 バンドル (`RelayCertificateBundleV2::to_cbor()`)。
  記述子リレー、機能フラグ、ML-KEM などのバンドルをサポート
  Ed25519/ML-DSA-65 です。

CLI の каждый バンドル против объявленных ключей の発行者 перед объединением
スナップショット。

Вызывайте CLI с `--guard-directory`、чтобы объединить актуальный consensus с
キャッシュ。ガードをセレクターで保護し、セキュリティを強化します
ディレクトリを参照してください。リレーが必要です。
フェッチ обновленный キャッシュ записывается по пути `--guard-cache`,
обеспечивая детерминированность следующих сессий。 SDK は、
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
`GuardSet` と `SorafsGatewayFetchOptions` を確認してください。`ml_kem_public_hex` セレクターの PQ ガードを確認します
SNNet-5 をロールアウトします。ステージ切り替え (`anon-guard-pq`、`anon-majority-pq`、
`anon-strict-pq`) リレー: когда
PQ ガード、セレクター、クラシック ピン、чтобы последующие
握手もできるよ。 CLI/SDK の概要
`anonymity_status`/`anonymity_reason`、`anonymity_effective_policy`、
`anonymity_pq_selected`、`anonymity_classical_selected`、`anonymity_pq_ratio`、
`anonymity_classical_ratio` は供給、
ブラウンアウトと古典的なフォールバックの両方。

SRCv2 バンドルのガード ディレクトリ
`certificate_base64`。 Оркестратор декодирует каждый バンドル、повторно проверяет
Ed25519/ML-DSA とガード キャッシュをサポートします。
Когда сертификат присутствует、он становится каноническим источником PQ キー、
握手とメッセージ。 просроченные сертификаты отбрасываются, и selector
жизненным циклом 回路と доступны через `telemetry::sorafs.guard` および
`telemetry::sorafs.circuit`, фиксируя окно валидности, ハンドシェイク スイート и
наличие двойных подписей для каждого ガード。

CLI ヘルパー、スナップショットの作成などの機能を提供します。
意味:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` は、SRCv2 スナップショットを取得し、`verify` を取得します。
パイプライン валидации для артефактов из других команд, выдавая JSON
概要、出力ガード セレクター CLI/SDK の説明。

### 1.5 Менеджер жизненного цикла 回路

リレー ディレクトリ、ガード キャッシュ、サーキットなどの機能
ライフサイクル マネージャー ソリューション、SoraNet 回線
フェッチします。 Конфигурация находится в `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) 日付:

- `relay_directory`: SNNet-3 ディレクトリのスナップショット、中間/出口ホップ
  выбирались детерминированно。
- `circuit_manager`: опциональная конфигурация (включена по умолчанию),
  TTL と呼ばれます。

Norito JSON は `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK ディレクトリのディレクトリ
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)、CLI および автоматически、когда
`--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`)。

Менеджер обновляет サーキット、когда меняются метаданные ガード (エンドポイント、PQ キー)
固定されたタイムスタンプ) および TTL。 `refresh_circuits`、Хелпер `refresh_circuits`、
フェッチ (`crates/sorafs_orchestrator/src/lib.rs:1346`)、フェッチ (`crates/sorafs_orchestrator/src/lib.rs:1346`)、フェッチ
`CircuitEvent`, позволяя операторам отслеживать резволя жизненного цикла.浸す
тест `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) 日付が表示されます。
латентность на трех ローテーションガード。 Потрите отчет в
`docs/source/soranet/reports/circuit_stability.md:1`。

### 1.6 クイッククイック実行

Оркестратор может опционально запускать локальный QUIC-прокси, чтобы браузерные
SDK では、キャッシュ キーをガードします。 Прокси
ループバックを実行し、QUIC を実行し、Norito マニフェストを実行します。
ガード キャッシュ キー。交通手段、
эмитируемые прокси, учитываются в `sorafs_orchestrator_transport_events_total`.

Включите прокси через новый блок `local_proxy` в JSON :

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```- `bind_addr` для эфемерного
  ）。
- `telemetry_label` распространяется в метриках, чтобы даборды различали прокси
  и fetch-сессии。
- `guard_cache_key_hex` (опционально) позволяет прокси отдавать тот же キー付き
  ガード キャッシュ、CLI/SDK のインターフェイス、セキュリティ インターフェイス
  синхронизированными。
- `emit_browser_manifest` включает выдачу マニフェスト、который распоирения могут
  Похранять и проверять。
- `proxy_mode` выбирает, будет ли прокси мостить трафик локально (`bridge`) или
  только отдавать метаданные, чтобы SDK открывали SoraNet 回路 сами
  (`metadata-only`)。 По умолчанию `bridge`; `metadata-only` を参照してください。
  マニフェストを確認してください。
- `prewarm_circuits`、`max_streams_per_circuit`、`circuit_ttl_hint_secs`
  ヒント、ヒント、ヒントを得るために、ヒントを見つけてください。
  回路を再利用できます。
- `car_bridge` (опционально) キャッシュ CAR-архивов です。 Поле
  `extension` задает суффикс, добавляемый когда target не содержит `*.car`;ということで
  `allow_zst = true` は `*.car.zst` です。
- `kaigi_bridge` (опционально) экспонирует Kaigi ルートとスプール。 Поле
  `room_policy` объявляет режим `public` または `authenticated`、чтобы браузерные
  GAR ラベルを追加します。
- `sorafs_cli fetch` は `--local-proxy-mode=bridge|metadata-only` をオーバーライドします
  `--local-proxy-norito-spool=PATH`、ランタイムを実行します。
  スプールは JSON 形式で表示されます。
- `downgrade_remediation` (опционально) ダウングレード フック。
  テレメトリ リレーのダウングレードと、
  `threshold` と `window_secs` を確認してください。
  `target_mode` (`metadata-only`)。 Когда は прекращаются をダウングレードします。
  `resume_mode` と `cooldown_secs` を確認してください。 Используйте массив
  `modes`、чтобы ограничить триггер конкретными ролями リレー (по умолчанию エントリーリレー)。

橋からの距離は、次のとおりです。

- **`norito`** — ストリーム ターゲット クラス относительно
  `norito_bridge.spool_dir`。ターゲット санитизируются (トラバーサル、トラバーサル)
  абсолютных путей)、и если файл без раслирения、применяется настроенный суффикс
  ペイロードを確認してください。
- **`car`** — ストリーム ターゲット、`car_bridge.cache_dir`、наследуют
  ペイロード、`allow_zst` が表示されます。
  Успезный ブリッジ отвечает `STREAM_ACK_OK` перед передачей байтов архива, чтобы
  パイプラインの検証。

В обоих случаях прокси предоставляет HMAC キャッシュ タグ (ガード キャッシュ キー)
ハンドシェイク) および `norito_*` / `car_*` 理由コード、メッセージ
最高です、отсутствие файлов и обки санитизации。

`Orchestrator::local_proxy().await` ハンドル モジュール PEM
ブラウザのマニフェストを確認してください。
приложения。

**マニフェスト v2** を確認してください。 Помимо
ガード キャッシュ キー、v2 のバージョン:

- `alpn` (`"sorafs-proxy/1"`) と `capabilities`、чтобы клиенты знали
  ぱんぱん。
- `session_id` ハンドシェイクと `cache_tagging` ソルト ブロックの派生
  セッション ガード アフィニティと HMAC タグ。
- ヒント、回路、ガード選択 (`circuit`、`guard_selection`、
  `route_hints`) UI が表示されます。
- `telemetry_v2` は、ノブを調整します。
- Каждый `STREAM_ACK_OK` または `cache_tag_hex`。 Клиенты отражают это значение
  `x-sorafs-cache-tag` と HTTP/TCP のキャッシュ ガード
  セレクションを選択してください。

Эти поля обратно совместимы — старые клиенты могут игнорировать новые ключи и
v1 サブセットです。

## 2. Семантика отказов

Оркестратор применяет строгие проверки возможностей и бюджетов до передачи
ピント。 Отказы делятся на три категории:1. **適格 (飛行前)。** 射程距離能力、
   広告とスコアボードの表示
   Артефакте и не попадают в планирование. CLI の概要に関する説明
   `ineligible_providers` причинами, чтобы операторы могли увидеть ドリフト
   ガバナンスは重要です。
2. **実行時枯渇。** 実行時に枯渇します。
   Когда достигается `provider_failure_threshold`, провайдер помечается как
   `disabled` до конца сессии。 Если провайдеры стали `disabled`, оркестратор
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }` を参照してください。
3. **Детерминированные прерывания.** Жесткие лимиты поднимаются как структурные
   और देखें
   - `MultiSourceError::NoCompatibleProviders` — スパン/アライメント、
     который оставлюся провайдеры не могут соблюсти.
   - `MultiSourceError::ExhaustedRetries` — 予算のチャンク。
   - `MultiSourceError::ObserverFailed` — ダウンストリーム オブザーバー (ストリーミング フック)
     отклонили проверенный チャンク。

Каждая озибка содержит индекс проблемного chunk и, когда доступно, финальную
причину отказа провайдера.ブロッカーをリリースする — повторные
попытки с тем же input воспроизведут сбой, пока advert, телеметрия или здоровье
そうです。

### 2.1 スコアボード

При настройке `persist_path` оркестратор записывает финальный スコアボード после
каждого прогона. JSON 形式:

- `eligibility` (`eligible` または `ineligible::<reason>`)。
- `weight` (нормализованный вес、назначенный для этого прогона)。
- メソッド `provider` (エンドポイント、エンドポイント)。

スナップショット スコアボード、リリース、リリースのスナップショット
ブラックリストとロールアウトが必要です。

## 3. Телеметрия と отладка

### 3.1 Метрики Prometheus

Оркестратор эмитит следующие метрики через `iroha_telemetry`:

| Метрика |ラベル | Описание |
|----------|----------|----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`、`region` |ゲージをフェッチします。 |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`、`region` |フェッチを取得します。 |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`、`region`、`reason` | Счетчик финальных отказов (исчерпаны ретраи, нет провайдеров, озибка オブザーバー)。 |
| `sorafs_orchestrator_retries_total` | `manifest_id`、`provider`、`reason` | Счетчик попыток ретраев по провайдерам. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`、`provider`、`reason` | Счетчик провайдерских отказов на уровне сессии, приводящих к отключению. |
| `sorafs_orchestrator_policy_events_total` | `region`、`stage`、`outcome`、`reason` |ロールアウトとフォールバックの両方をサポートします。 |
| `sorafs_orchestrator_pq_ratio` | `region`、`stage` | PQ リレーは SoraNet から送信されます。 |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`、`stage` | PQ リレーとスナップショット スコアボードを表示します。 |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`、`stage` | Гистограмма дефицита политики (разница между целью и фактической долей PQ)。 |
| `sorafs_orchestrator_classical_ratio` | `region`、`stage` | Гистограмма доли классических リレー в каждой сессии。 |
| `sorafs_orchestrator_classical_selected` | `region`、`stage` | Гистограмма числа выбранных классических リレー в сессии。 |

ステージング ダッシュボードとプロダクション ノブを組み合わせます。
SF-6 のバージョン:

1. **アクティブなフェッチ** — 完了を確認するためのゲージ。
2. **再試行率** — ベースライン、`retry`。
3. **プロバイダーの障害** — триггерит ポケベル アラート、когда любой провайдер превылает
   `session_failure > 0` から 15 分です。

### 3.2 構造化ログのターゲット

ターゲットの例:

- `telemetry::sorafs.fetch.lifecycle` — `start` および `complete` の値
  チャンク、ретраев и общей длительностью。
- `telemetry::sorafs.fetch.retry` — テスト (`provider`、`reason`、
  `attempts`) トリアージ。
- `telemetry::sorafs.fetch.provider_failure` — провайдеры、отключенные из-за
  और देखें
- `telemetry::sorafs.fetch.error` — финальные отказы с `reason` и опциональными
  そうです。Направляйте эти потоки в существующий Norito ログ パイプライン、インシデントが発生しました
応答 был единый источник истины。 PQ/クラシックのライフサイクル
ミックス `anonymity_effective_policy`、`anonymity_pq_ratio`、
`anonymity_classical_ratio` и связанные счетчики, что упрощает настройку
дабордов без парсинга метрик。 GA のロールアウトが完了しました
`info` ライフサイクル/再試行 `warn` ターミナル エラー。

### 3.3 JSON の概要

`sorafs_cli fetch` と Rust SDK の要約、要約:

- `provider_reports` を確認してください。
- `chunk_receipts`、показывающий какой провайдер обслужил каждый チャンク。
- `retry_stats` または `ineligible_providers`。

要約と領収書 - 領収書
соотносятся с лог-метаданными выbolе。

## 4. Операционный чеклист

1. **Задеплойте конфигурацию в CI.** Запустите `sorafs_fetch` с целевой
   конфигурацией, передайте `--scoreboard-out` для фиксации eligibility-view и
   リリースが完了しました。 Любой неожиданный ineligible провайдер
   そうです。
2. **Проверьте телеметрию.** Убедитесь、что деплой экспортирует метрики
   `sorafs.fetch.*` は、マルチソースフェッチを実行します。
   ああ、それは。 Отсутствие метрик обычно означает, что фасад оркестратора
   そうだね。
3. **Документируйте がオーバーライドされます。** 緊急 `--deny-provider` および
   `--boost-provider` は JSON (CLI と変更ログ) を組み合わせたものです。ロールバック
   スコアボードのスナップショットを上書きします。
4. **煙テストを実行します。** 予算と上限を設定します。
   フィクスチャをフェッチする
   (`fixtures/sorafs_manifest/ci_sample/`) и убедитесь、領収書とチャンク
   остаются детерминированными。

舞台化されたステージでのパフォーマンス
ロールアウトとインシデント対応の両方。

### 4.1 オーバーライド

Операторы могут закрепить активный 輸送/匿名性 этап без изменения базовой
конфигурации, задав `policy_override.transport_policy` и
`policy_override.anonymity_policy` と JSON `orchestrator` (つまり
`--transport-policy-override=` / `--anonymity-policy-override=` ×
`sorafs_cli fetch`)。 Если オーバーライド присутствует、оркестратор пропускает обычный
ブラウンアウト フォールバック: если требуемый PQ tier недостижим、fetch завербуемый с
`no providers` ダウングレード。 Возврат к поведению по умолчанию —
上書きすることができます。