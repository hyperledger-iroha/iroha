---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d948fcd78a564487591aeba23d4587de337913984fd3a5861a83f2a9a23887d9
source_last_modified: "2026-01-05T09:28:11.855022+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-conformance
title: SoraFS Chunker Conformance Guide
sidebar_label: Chunker Conformance
description: Requirements and workflows for preserving the deterministic SF1 chunker profile across fixtures and SDKs.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
:::

Бұл нұсқаулық әр іске асырудың сақталуы үшін орындауы керек талаптарды кодтайды
SoraFS детерминирленген chunker профилімен (SF1) тураланған. Ол да
қалпына келтіру жұмыс үрдісін, қол қою саясатын және тексеру қадамдарын құжаттайды
SDK арқылы арматура тұтынушылары синхрондалған күйде қалады.

## Канондық профиль

- Профиль тұтқасы: `sorafs.sf1@1.0.0`
- Енгізу тұқымы (он алтылық): `0000000000dec0ded`
- Мақсатты өлшем: 262144 байт (256КБ)
- Ең аз өлшем: 65536 байт (64КБ)
- Ең үлкен өлшем: 524288 байт (512КБ)
- Айналмалы көпмүшелік: `0x3DA3358B4DC173`
- Беріліс үстелінің тұқымы: `sorafs-v1-gear`
- Үзіліс маскасы: `0x0000FFFF`

Анықтаманың орындалуы: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Кез келген SIMD жеделдету бірдей шекаралар мен дайджесттерді шығаруы керек.

## Арматура жинағы

`cargo run --locked -p sorafs_chunker --bin export_vectors` қалпына келтіреді
`fixtures/sorafs_chunker/` астында келесі файлдарды бекітеді және шығарады:

- `sf1_profile_v1.{json,rs,ts,go}` — Rust үшін канондық бөлік шекаралары,
  TypeScript және Go тұтынушылары. Әрбір файл канондық дескриптор ретінде жарнамалайды
  `profile_aliases` ішіндегі бірінші (және жалғыз) жазба. Бұйрықты жүзеге асырады
  `ensure_charter_compliance` және өзгертуге БОЛМАЙДЫ.
- `manifest_blake3.json` — әрбір арматура файлын қамтитын BLAKE3 арқылы тексерілген манифест.
- `manifest_signatures.json` - Манифесттегі кеңес қолдары (Ed25519)
  қорыту.
- `sf1_profile_v1_backpressure.json` және `fuzz/` ішіндегі өңделмеген корпус —
  Chunker кері қысым сынақтары арқылы пайдаланылатын детерминирленген ағындық сценарийлер.

### Қол қою саясаты

Арматураны қалпына келтіру **міндетті түрде** жарамды кеңес қолын қамтуы керек. Генератор
`--allow-unsigned` анық берілмейінше, қол қойылмаған шығысты қабылдамайды (арналған
тек жергілікті эксперимент үшін). Қолтаңба конверттері тек қосымша және
әр қол қоюшы үшін қайталанатын.

Кеңес қолтаңбасын қосу үшін:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Тексеру

`ci/check_sorafs_fixtures.sh` CI көмекшісі генераторды қайта ойнатады
`--locked`. Бекіткіштер жылжып кетсе немесе қолтаңбалар жоқ болса, тапсырма орындалмайды. Қолдану
бұл сценарийді түнгі жұмыс үрдістерінде және арматура өзгерістерін жібермес бұрын.

Қолмен тексеру қадамдары:

1. `cargo test -p sorafs_chunker` іске қосыңыз.
2. `ci/check_sorafs_fixtures.sh` жергілікті түрде шақырыңыз.
3. `git status -- fixtures/sorafs_chunker` таза екенін растаңыз.

## Ойын кітабын жаңарту

Жаңа chunker профилін ұсынғанда немесе SF1 жаңартқанда:

Сондай-ақ қараңыз: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) үшін
метадеректер талаптары, ұсыныс үлгілері және тексеруді тексеру тізімдері.

1. Жаңа параметрлері бар `ChunkProfileUpgradeProposalV1` (RFC SF‑1 қараңыз) жобасын жасаңыз.
2. `export_vectors` арқылы арматураларды қалпына келтіріп, жаңа манифест дайджестін жазыңыз.
3. Қажетті кеңес кворумымен манифестке қол қойыңыз. Барлық қолдар болуы керек
   `manifest_signatures.json` қосымшасына қосылған.
4. Зардап шеккен SDK құрылғыларын жаңартыңыз (Rust/Go/TS) және жұмыс уақытының арасындағы теңдікке көз жеткізіңіз.
5. Параметрлер өзгерсе, fuzz corpora қалпына келтіріңіз.
6. Осы нұсқаулықты жаңа профиль тұтқасымен, тұқымдармен және дайджестпен жаңартыңыз.
7. Өзгерістерді жаңартылған сынақтармен және жол картасының жаңартуларымен бірге жіберіңіз.

Бұл процесті орындамай, бөлік шекараларына немесе дайджесттерге әсер ететін өзгерістер
жарамсыз және біріктірілмеуі керек.