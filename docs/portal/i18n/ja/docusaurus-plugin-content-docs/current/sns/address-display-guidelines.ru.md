---
lang: ja
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ExplorerAddressCard を '@site/src/components/ExplorerAddressCard' からインポートします。

:::note Канонический источник
Эта страница отражает `docs/source/sns/address_display_guidelines.md` и теперь
служит канонической копией портала. Исходный файл остается для PR переводов.
:::

Кобозреватели и примеры SDK должны относиться к адресам аккаунтов как
ペイロード。 Android の機能
`examples/android/retail-wallet` のユーザー エクスペリエンス:

- **Две цели копирования.** Предоставьте две явные кнопки копирования: IH58
  (предпочтительно) и сжатая Sora のみのформа (`sora...`, второй по предпочтению)。 IH58 のレビュー
  QR ペイロードを確認してください。 Сжатая форма должна включать
  встроенное предупреждение, потому что работает только в Sora-aware приложениях.
  Android のマテリアルとツールチップの機能
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`、а
  iOS SwiftUI と互換性のある UX と `AddressPreviewCard` が互換性があります。
  `examples/ios/NoritoDemo/Sources/ContentView.swift`。
- **Моносовывайте обе строки моносиринным
  с `textIsSelectable="true"`、чтобы пользователи могли проверять
  IME を使用します。 Избегайте редактируемых полей: IME может переписать
  かな、или внедрить нулевой зирины кодовые точки。
- **Подсказки для неявного домена по умолчанию.** Когда селектор указывает на
  неявный домен `default`, показывайте подпись, напоминающую операторам, что
  суффикс не требуется。 Обозреватели также должны выделять канонический доменный
  ярлык、когда селектор кодирует ダイジェスト。
- **QR для IH58.** QR-коды должны кодировать строку IH58. Если генерация QR
  провалилась, покажите явную обкувместо пустого изображения。
- **Сообщение буфера обмена.** После копирования сжатой формы отправьте トースト
  スナックバー、ソラのみのスナックバー、пользователям、что она
  IME。

ガードレールのサポート Unicode/IME と удовлетворяет
критериям приемки дорожной карты ADDR-6 для UX кобозревателей。

## Скринсоты для сверки

Используйте следующие фикстуры при локализационных проверках, чтобы подписи
ツールチップとツールチップの表示:

- Android 版: `/img/sns/address_copy_android.svg`

  ![Справка Android двойного копирования](/img/sns/address_copy_android.svg)

- iOS 版: `/img/sns/address_copy_ios.svg`

  ![Справка iOS двойного копирования](/img/sns/address_copy_ios.svg)

## Помощники SDK

Каждый SDK предоставляет удобный ヘルパー、возвращающий формы IH58、сжатую、а
UI-слои оставались согласованными:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript インスペクター: `inspectAccountId(...)` を参照してください。
  сжатой форме и добавляет его в `warnings`, когда вызывающие передают literal
  `sora...`、чтобы обозреватели/панели кольков могли показывать предупреждение
  ソラのみの во время вставки/валидации, а не только при генерации сжатой формы.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- スイフト: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

UI のエンコードをサポートするヘルパーです。
JavaScript ヘルパーのペイロード `selector` と `domainSummary`
(`tag`、`digest_hex`、`registry_id`、`label`)、чтобы UI могли указать、является
Local-12 では、レジストリ、ペイロード、ペイロードが表示されます。

## Демонстрация инструментирования обозревателя

<エクスプローラーアドレスカード />

Обозреватели должны отражать телеметрию и доступность колелька:- Добавляйте `data-copy-mode="ih58|compressed|qr"` на кнопки копирования, чтобы
  фронтенды могли эмитить счетчики использования рядом с метрикой Torii
  `torii_address_format_total`。 Демонстрационный компонент выге отправляет
  событие `iroha:address-copy` с `{mode,timestamp}` - подключите его к своему
  аналитическому/телеметрийному пайплайну (например, в Segment или NORITO)
  коллектор)、ダッシュボードのダッシュボード использование форматов
  адресов на сервере с режимами копирования клиента. Также зеркальте счетчики
  Torii (`torii_address_domain_total{domain_kind}`) のメッセージ
  проверки вывода Local-12 могли экспортировать 30-дневное доказательство
  `domain_kind="local12"` は、Grafana `address_ingest` です。
- Сопоставляйте каждому контролу отдельные `aria-label`/`aria-describedby`、
  объясняющие, безопасен ли リテラル для обмена (IH58) или только для Sora
  (жатый)。 Включайте подпись неявного домена в описание, чтобы вспомогательные
  технологии показывали тот же контекст.
- Экспонируйте live-регион (например、`<output aria-live="polite">...</output>`)、
  который объявляет результаты копирования и предупреждения, совпадая с
  VoiceOver/TalkBack、Swift/Android と互換性があります。

Это инструментирование удовлетворяет ADDR-6b、показывая、что операторы могут
наблюдать и прием Torii, и режимы копирования клиента до отключения ローカル
さい。

## ローカル -> グローバル

[ローカル -> グローバル ツールキット](local-to-global-toolkit.md) のバージョン
ローカルのセキュリティーを確認してください。ヘルパー
JSON-отчет аудита、и конвертированный список IH58/сжатых значений、который
準備状況を確認し、ランブックを作成します。
Grafana および Alertmanager、厳格なカットオーバー。

## Быстрый справочник бинарной раскладки (ADDR-1a)

SDK の機能 (インスペクター、機能)
валидации、マニフェスト ビルダー)、направляйте разработчиков к каноническому
ワイヤーは `docs/account_structure.md` です。 Раскладка всегда
`header · selector · controller`、ヘッダーの内容:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (ビット 7 ～ 5) ненулевые значения зарезервированы и
  должны приводить к `AccountAddressError::InvalidHeaderVersion`。
- `addr_class` シングル (`0`) とマルチシグ (`1`) の組み合わせ。
- `norm_version = 1` кодирует правила селектора Norm v1. Будущие нормы будут
  2 番目に表示されます。
- `ext_flag` と `0`; установленные биты означают неподдерживаемые расзирения
  ペイロード。

ヘッダーの内容:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI と SDK の組み合わせ:

- `0x00` = неявный домен по умолчанию (ペイロード)。
- `0x01` = локальный ダイジェスト (12 バイト `blake2s_mac("SORA-LOCAL-K:v1", label)`)。
- `0x02` = レジストリ (`registry_id:u32` ビッグエンディアン)。

Канонические hex-примеры, которые инструменты кольков могут ссылать или
ドキュメント/テストの結果:

| Тип селектора | Канонический 16 進数 |
|---------------|---------------|
| Неявный по умолчанию | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Локальный ダイジェスト (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Глобальный レジストリ (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

См. `docs/source/references/address_norm_v1.md` セレクタ/状態
と `docs/account_structure.md` が表示されます。

## Принуждение канонических форм

Операторы、конвертирующие устаревbolие ローカル кодировки в канонический IH58 или
CLI-флоу および ADDR-5 を使用する場合:

1. `iroha tools address inspect` теперь выдает структурированное JSON-резюме с IH58,
   圧縮された 16 進ペイロード。 Резюме также включает объект `domain`
   `kind`/`warning` と отражает любой переданный домен через
   `input_domain`。 `kind` の `local12`、CLI が表示されます。
   stderr、JSON-резюме отражает ту же подсказку、чтобы CI пайплайны и SDK могли
   そうですね。 `--append-domain` を参照してください。
   конвертированный エンコーディング как `<ih58>@<domain>`。
2. SDK の警告/概要 JavaScript ヘルパー:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  ヘルパー сохраняет IH58 префикс、извлеченный из リテラル、если только вы явно не
  указали `networkPrefix`, поэтому резюме для не-default сетей не перерисовываются
  тихо с дефолтным префиксом.3. Конвертируйте канонический ペイロード、повторно используя `ih58.value` или
   `compressed` は、`--format` です。 Эти
   же безопасны для внего обмена.
4. マニフェスト、レジストリ、およびそれらのオブジェクト
   ローカルのカットオーバーを実行します。
5. あなたのことを忘れないでください。
   `iroha tools address audit --input addresses.txt --network-prefix 753`。 Команда
   читает リテラル、разделенные переводом строки (комментарии с `#` игнорируются,
   `--input -` または STDIN)、JSON-отчет с
   каноническими/IH58/сжатыми резюме для каждой записи и считает обки парсинга
   и предупреждения ローカルな日。 `--allow-errors` を確認してください
   `--fail-on-warning`、ローカル セキュリティと CI。
6. Когда нужна построчная перепись, используйте
  ローカル修復を行う
  CSV `input,status,format,...`、который подсвечивает канонические
  кодировки、предупреждения и озибки парсинга за один проход。
   ヘルパーは、ローカル チームをサポートし、ローカル チームをサポートします。
   エンコーディング (IH58/сжатый/hex/JSON) と сохраняет исходный домен の組み合わせ
   `--append-domain`。 Совмещайте с `--allow-errors`, чтобы продолжать скан даже
   リテラルをダンプします。
7. CI/lint メソッド `ci/check_address_normalize.sh`, которая
   ローカル セキュリティ `fixtures/account/address_vectors.json`、
   конвертирует их через `iroha tools address normalize`, и повторно запускает
   `iroha tools address audit --fail-on-warning`, чтобы доказать, что релизы больге не
   ローカルダイジェスト。

`torii_address_local8_total{endpoint}` 最高です
`torii_address_collision_total{endpoint,kind="local12_digest"}`、
`torii_address_collision_domain_total{endpoint,domain}` または Grafana
`dashboards/grafana/address_ingest.json` 施行日: когда
ローカル отправок и ноль ローカル 12
30 日以内に、Torii がローカル 8 ゲートとハード フェイル メッセージを表示します。
メインネット、ローカル 12、ローカル 12 からのアクセスが可能です。
レジストリ。 Считайте вывод CLI операторским уведомлением об этом замораживании -
SDK ツールチップと автоматизации, чтобы の最新情報
Сохранять паритет с критериями выхода дорожной карты. Torii теперь по умолчанию
Аластерах при диагностике регрессий。 Продолжайте зеркалировать
`torii_address_domain_total{domain_kind}` × Grafana
(`dashboards/grafana/address_ingest.json`)、ADDR-7 を使用してください。
Пог подтвердить, что `domain_kind="local12"` оставался нулевым в течение
アラートマネージャー (`dashboards/alerts/address_ingest_rules.yml`) のメッセージ
ガードレール:

- `AddressLocal8Resurgence` пейджит、когда контекст сообщает о свежем инкременте
  ローカル-8。厳密モードのロールアウト、SDK の使用、および、
  Созврата сигнала к нулю - затем восстановите дефолт (`true`).
- `AddressLocal12Collision` срабатывает、когда два Local-12 лейбла хэзируются в
  один ダイジェスト。プロモーション マニフェストの詳細、ローカル -> グローバル
  ツールキットとマッピング ダイジェストと Nexus ガバナンス ツールキット
  レジストリ エントリとダウンストリーム ロールアウトを確認します。
- `AddressInvalidRatioSlo` предупреждает、когда флотский 無効な比率 (исключая)
  Local-8/strict-mode) では SLO 0.1% が発生します。 Используйте
  `torii_address_invalid_total` は、/причины ответственного контекста/причины и
  SDK は厳密モードで使用できます。

### Фрагмент релизной заметки (когмелек и обозреватель)

リリースノートのリリースノートのカットオーバー:

> **Адреса:** ヘルパー `iroha tools address normalize --only-local --append-domain`
> и подключен в CI (`ci/check_address_normalize.sh`)、чтобы пайплайны козелька/
> обозревателя могли конвертировать устаревbolие ローカル селекторы в канонические
> IH58/ローカル-8/ローカル-12 メインネット。 Обновите любые
> кастомные экспорты, чтобы запускать команду и прикладывать нормализованный
> 証拠バンドルが必要です。