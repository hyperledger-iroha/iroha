---
lang: hy
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito Հոսքային

Norito Streaming-ը սահմանում է մետաղալարերի ձևաչափը, կառավարման շրջանակները և հղման կոդեկը
օգտագործվում է ուղիղ մեդիա հոսքերի համար Torii-ով և SoraNet-ով: Կանոնական առանձնահատկությունն ապրում է
`norito_streaming.md` աշխատանքային տարածքի արմատում; այս էջը թորում է այն կտորները, որոնք
օպերատորներին և SDK-ի հեղինակներին անհրաժեշտ են կազմաձևման հպման կետերը:

## Լարի ձևաչափը և կառավարման հարթությունը

- **Դրսևորումներ և շրջանակներ** `ManifestV1` և `PrivacyRoute*` նկարագրում են հատվածը
  ժամանակագրություն, կտորների նկարագրիչներ և երթուղու ակնարկներ: Կառավարման շրջանակներ (`KeyUpdate`,
  `ContentKeyUpdate` և cadence feedback) ապրում են մանիֆեստի հետ միասին
  դիտողները կարող են վավերացնել պարտավորությունները նախքան վերծանումը:
- **Հիմնական կոդեկ.** `BaselineEncoder`/`BaselineDecoder` ուժի մեջ է միապաղաղ
  կտորի ID-ներ, ժամանակի դրոշմանիշի թվաբանություն և պարտավորությունների ստուգում: Տանտերերը պետք է զանգահարեն
  `EncodedSegment::verify_manifest` հեռուստադիտողներին կամ ռելեներին սպասարկելուց առաջ:
- **Հատկանիշի բիթ.** Հնարավորությունների բանակցությունները գովազդում են `streaming.feature_bits`
  (կանխադրված `0b11` = ելակետային հետադարձ կապ + գաղտնիության երթուղու մատակարար) այնպես որ փոխանցումներ և
  հաճախորդները կարող են մերժել հասակակիցներին՝ առանց դետերմինիստականորեն համապատասխանելու հնարավորությունների:

## Բանալիներ, սյուիտներ և արագություն

- **Ինքնության պահանջներ:** Հոսքի կառավարման շրջանակները միշտ ստորագրված են
  Ed25519. Նվիրված բանալիները կարող են մատակարարվել միջոցով
  `streaming.identity_public_key`/`streaming.identity_private_key`; հակառակ դեպքում
  հանգույցի ինքնությունը կրկին օգտագործվում է:
- **HPKE հավաքակազմ.** `KeyUpdate`-ն ընտրում է ամենացածր ընդհանուր փաթեթը; Սյուիտ թիվ 1 է
  պարտադիր (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`),
  կամընտիր `Kyber1024` արդիականացման ուղի: Սյուիտի ընտրությունը պահվում է
  նիստը և վավերացվում է յուրաքանչյուր թարմացման ժամանակ:
- **Ռոտացիա.** Հրատարակիչները թողարկում են ստորագրված `KeyUpdate` յուրաքանչյուր 64 ՄԲ կամ 5 րոպեն մեկ:
  `key_counter`-ը պետք է խստորեն ավելանա. հետընթացը ծանր սխալ է:
  `ContentKeyUpdate`-ը տարածում է շարժվող խմբի բովանդակության բանալին՝ փաթաթված տակով
  բանակցված HPKE փաթեթը և դարպասների հատվածի վերծանումը ID + վավերականությամբ
  պատուհան.
- **Պատկերներ.** `StreamingSession::snapshot_state` և
  `restore_from_snapshot` persist `{session_id, key_counter, suite, sts_root,
  cadence state}` under `streaming.session_store_dir` (կանխադրված
  `./storage/streaming`): Տրանսպորտային բանալիները նորից ստացվում են վերականգնման ժամանակ, ուստի խափանում է
  մի արտահոսեք նիստի գաղտնիքները:

## Runtime կոնֆիգուրացիա

- **Հիմնական նյութ:** Տրամադրել հատուկ ստեղներ
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519
  multihash) և կամընտիր Kyber նյութի միջոցով
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`. Բոլոր չորսը պետք է լինեն
  ներկա լինել, երբ գերակայում է լռելյայն; `streaming.kyber_suite` ընդունում է
  `mlkem512|mlkem768|mlkem1024` (մականունը՝ `kyber512/768/1024`, լռելյայն
  `mlkem768`):
- **Codec guardrails.** CABAC-ը մնում է անջատված, քանի դեռ կառուցվածքը դա թույլ չի տալիս;
  փաթեթավորված rans-ը պահանջում է `ENABLE_RANS_BUNDLES=1`: Կիրառել միջոցով
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` և ընտրովի
  `streaming.codec.rans_tables_path` մաքսային սեղաններ մատակարարելիս: Միավորված
- **SoraNet երթուղիներ.** `streaming.soranet.*`-ը վերահսկում է անանուն տրանսպորտը.
  `exit_multiaddr` (կանխադրված `/dns/torii/udp/9443/quic`), `padding_budget_ms`
  (կանխադրված 25 ms), `access_kind` (`authenticated` vs `read-only`), կամընտիր
  `channel_salt`, `provision_spool_dir` (կանխադրված
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (կանխադրված 0,
  անսահմանափակ), `provision_window_segments` (կանխադրված 4) և
  `provision_queue_capacity` (կանխադրված 256):
- **Sync gate.** `streaming.sync`-ը միացնում է դրեյֆի կիրառումը աուդիովիզուալ համար
  հոսքեր՝ `enabled`, `observe_only`, `ewma_threshold_ms` և `hard_cap_ms`
  կառավարել, երբ հատվածները մերժվում են ժամանակի շեղումների պատճառով:

## Վավերացում և հարմարանքներ

- Կանոնական տիպի սահմանումներ և օգնականներ ապրում են
  `crates/iroha_crypto/src/streaming.rs`.
- Ինտեգրման ծածկույթն իրականացնում է HPKE-ի ձեռքսեղմումը, բովանդակության բանալիների բաշխումը,
  և լուսանկարի կյանքի ցիկլը (`crates/iroha_crypto/tests/streaming_handshake.rs`):
  Գործարկեք `cargo test -p iroha_crypto streaming_handshake`՝ հոսքը ստուգելու համար
  մակերեսը տեղայնորեն:
- Դասավորության, սխալների մշակման և ապագա թարմացումների խորը սուզման համար կարդացեք
  `norito_streaming.md` պահեստի արմատում: