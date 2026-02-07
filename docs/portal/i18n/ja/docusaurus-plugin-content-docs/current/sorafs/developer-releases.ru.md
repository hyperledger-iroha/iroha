---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Процесс релиза
概要: CLI/SDK の説明、説明、メモ、メモ。
---

# Процесс релиза

SoraFS (`sorafs_cli`、`sorafs_fetch`、ヘルパー) および SDK クレート
(`sorafs_car`、`sorafs_manifest`、`sorafs_chunker`) です。 Релизный
パイプライン CLI と библиотеки согласованными、обеспечивает покрытие lint/test
と、下流にある。チェックリストを作成する
каждого 候補タグ。

## 0. サインオフを完了します

リリース ゲートのセキュリティ レビューを確認します。

- Скачайте самый свежий меморандум SF-6 の詳細 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  SHA256 ハッシュとリリース チケット。
- 修復チケット (например、`governance/tickets/SF6-SR-2026.md`) および отметьте を参照してください。
  承認-ответственных из セキュリティ エンジニアリング および ツール ワーキング グループ。
- 修正チェックリストを参照してください。 незакрытые пункты блокируют релиз。
- パリティ ハーネス (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  バンドルマニフェストです。
- Убедитесь、что команда подписи、которую вы планируете выполнить、включает и `--identity-token-provider`, и
  явный `--identity-token-audience=<aud>`、スコープのフルシオ был зафиксирован в релизных の証拠。

ガバナンスを強化する必要があります。

## 1. リリース/テストゲートを作成する

`ci/check_sorafs_cli_release.sh` と互換性のある、Clippy と тесты
CLI と SDK クレートのワークスペース ローカル ターゲット (`.target`) の機能
конфликтов прав при запуске внутри CI контейнеров.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Скрипт выполняет следующие проверки:

- `cargo fmt --all -- --check` (ワークスペース)
- `cargo clippy --locked --all-targets` と `sorafs_car` (機能 `cli`)、
  `sorafs_manifest` および `sorafs_chunker`
- `cargo test --locked --all-targets` для этих же 木箱

タグ付けが必要です。 Релизные сборки должны
メインです。チェリーピックとリリースの両方。ゲート также
キーレス署名機能 (`--identity-token-issuer`,
`--identity-token-audience`) там, где требуется; отсутствующие аргументы валят запуск。

## 2. Применить политику версионирования

SoraFS CLI/SDK クレートの SemVer:

- `MAJOR`: バージョン 1.0 です。 До 1.0 マイナー `0.y`
  CLI サーフェスと Norito の **означает Breaking изменения**。
  за опциональной политикой、добавления телеметрии）。
- `PATCH`: Исправления багов、ドキュメントのみの релизы и обновления зависимостей、
  не меняющие наблюдаемое поведение.

`sorafs_car`、`sorafs_manifest`、`sorafs_chunker` は、
ダウンストリーム SDK のバージョン文字列。例:

1. `version =` と `Cargo.toml` を比較します。
2. Перегенерируйте `Cargo.lock` через `cargo update -p <crate>@<new-version>` (ワークスペース)
   требует явных версий）。
3. リリースゲートを解除すると、ゲートが解除されます。

## 3. リリースノート

CLI、SDK でのマークダウン変更ログの更新
ガバナンス。 Используйте заблон `docs/examples/sorafs_release_notes.md` (скопируйте)
его в директорию релизных артефактов и заполните секции конкретикой)。

Минимальный набор:

- **ハイライト**: CLI と SDK の概要。
- **アップグレード手順**: TL;DR команды для обновления 貨物 зависимостей и перезапуска
  備品。
- **検証**: 封筒のメッセージが表示されます。
  `ci/check_sorafs_cli_release.sh`, которая была выполнена.

リリース ノート к тегу (GitHub リリース) を参照してください。
あなたのことを忘れないでください。

## 4. リリースフック

`scripts/release_sorafs_cli.sh`、署名バンドルを使用してください。
検証の概要、которые отгружаются с каждым релизом。ラッパー機能
CLI、`sorafs_cli manifest sign` および сразу воспроизводит
`manifest verify-signature`、タグ付けが必要です。例:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

説明:- Отслеживайте релизные 入力 (ペイロード、計画、概要、予想されるトークン ハッシュ)
  展開を確認し、必要な情報を確認してください。 CI
  バンドル под `fixtures/sorafs_manifest/ci_sample/` показывает канонический レイアウト。
- CI 自動化 - `.github/workflows/sorafs-cli-release.yml`; он выполняет
  リリース ゲート、バンドル/署名、ワークフロー、およびワークフロー。
  Повторяйте тот же порядок команд (ゲートを解除→署名→確認) в других CI системах,
  監査ログはハッシュと同じです。
- `manifest.bundle.json`、`manifest.sig`、`manifest.sign.summary.json` および
  `manifest.verify.summary.json` вместе - это пакет、на который ссылается ガバナンス通知。
- 正規フィクスチャ、マニフェスト、チャンク プランの標準化
  概要 в `fixtures/sorafs_manifest/ci_sample/` (и обновите)
  `docs/examples/sorafs_ci_sample/manifest.template.json`) タグ付け。下流側
  フィクスチャ リリース バンドルが含まれています。
- 制限付きチャネル `sorafs_cli proof stream` および
  ストリーミングを安全に保護します。
- リリース ノート、`--identity-token-audience`、リリース ノート。
  ガバナンスは視聴者を保護し、フルシオを保護します。

`scripts/sorafs_gateway_self_cert.sh`、ロールアウト ゲートウェイを参照してください。
マニフェスト バンドル、証明書、証明書、候補者:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Тегирование и публикация

チェックとフックの詳細:

1. `sorafs_cli --version` および `sorafs_fetch --version`、чтобы убедиться、что
   бинарники показывают новую версию。
2. リリース конфиг в `sorafs_release.toml` под контролем версий (предпочтительно)
   これにより、展開が完了します。 Избегайте
   アドホックなオプション。 CLI による `--config` (説明)
   入力を解放します。
3. Создайте подписанный тег (предпочтительно) или аннотированный тег:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Загрузите артефакты (CAR バンドル、マニフェスト、証明概要、リリース ノート、
   認証出力)、プロジェクト レジストリ、ガバナンス チェックリストなど
   [展開ガイド](./developer-deployment.md)。フィクスチャの追加、
   フィクスチャ リポジトリとオブジェクト ストア、監査自動化機能を備えています。
   バンドルをダウンロードしてください。
5. ガバナンスの概要、リリースノート、ハッシュ
   バンドル/バージョン マニフェスト、`manifest.sign/verify` 概要、および любые
   証明書の封筒。 Приложите URL CI ジョブ (или архив логов)、который выполнил
   `ci/check_sorafs_cli_release.sh` および `scripts/release_sorafs_cli.sh`。ガバナンスの徹底
   チケット、承認が必要です。クソ
   `.github/workflows/sorafs-cli-release.yml` публикует уведомления, связывайте
   ハッシュとアドホック サマリー。

## 6. Пострелизные действия

- Убедитесь、что документация、указывающая на новую версию (クイックスタート、CI テンプレート)、
  обновлена、либо подтвердите отсутствие изменений。
- ロードマップの詳細、移行の計画、
- ゲートを解除する - храните их рядом с подписанными
  Артефактами。

CLI、SDK クレート、ガバナンスなどのパイプラインの管理
ждом релизном цикле.