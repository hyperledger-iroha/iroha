---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-profile-authoring
כותרת: Guia de autoria de perfis de chunker da SoraFS
sidebar_label: Guia de autoria de chunker
תיאור: רשימת רשימת רשימת מאפיינים ופרפורמציות חדשות עבור chunker da SoraFS.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/chunker_profile_authoring.md`. Mantenha ambas as copias sincronizadas.
:::

# Guia de autoria de perfis de chunker da SoraFS

Este guia explica como propor e publicar novos perfis de chunker para a SoraFS.
השלמה ל-RFC de arquitetura (SF-1) ו-Reference do registro (SF-2a)
com requisitos concretos de autoria, etapas de validacao e modelos de proposta.
Para um exemplo canonico, veja
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o log de dry-run associado em
`docs/source/sorafs/reports/sf1_determinism.md`.

## Visao Geral

פרופיל לא ניתן לרשום:

- פרמטרים ידועים CDC קביעות והגדרות של multihash identicas entre
  arquiteturas;
- מכשירי entregar reproduziveis (JSON Rust/Go/TS + corpora fuzz + testemunhas PoR) que
  ערכות SDK של מערכת הפעלה במורד הזרם.
- incluir metadados prontos para governanca (מרחב שם, שם, semver) junto com orientacao
  de rollout e janelas operacionais; ה
- passar pela suite de diff determinista antes da revisao do conselho.

סיג רשימת בדיקה אבאיקסו להכנה אומה פרופוסטה que atenda a essas regras.

## רזומה דה קארטה לעשות רישום

Antes de redigir uma proposta, confirme que ela atende a carta do registro aplicada por
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- IDs de perfil sao inteiros positivos que aumentam de forma monotona sem lacunas.
- O handle canonico (`namespace.name@semver`) deve aparecer na list de alias e
  **פיתוח** סר אנטרדה ראשוני. כינויים אלטרנטיביים (לדוגמה, `sorafs.sf1@1.0.0`) vem depois.
- Nenhum alias pode colidir com outro handle canonico ou aparecer mais de uma vez.
- כינויים devem ser nao vazios e aparados de espacos em branco.

Helpers de CLI:

```bash
# Listagem JSON de todos os descritores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadados para um perfil default candidato (handle canonico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos mantem as propostas alinhadas com a carta do registro e fornecem os
metadados canonicos necessarios nas discussoes de governanca.

## דרישות המטאדוס| קמפו | תיאור | דוגמה (`sorafs.sf1@1.0.0`) |
|-------|--------|------------------------------|
| `namespace` | Agrupamento logico para perfis relacionados. | `sorafs` |
| `name` | Rotulo legivel para humanos. | `sf1` |
| `semver` | Cadeia de versao semantica para o conjunto de parametros. | `1.0.0` |
| `profile_id` | זיהוי מספרי מונוטוני אטריבוידו quando או פרפיל פנימי. שמור או פרוקסימו id mas nao לנצל מחדש את numeros existentes. | `1` |
| `profile_aliases` | מטפל ב-adicionais opcionais (nomes alternativos, abreviacoes) expostos a clientes durante a negociacao. כולל סמפר או ידית קנוניקו como primeira entrada. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | תוספת מינימלית לעשות נתח בבתים. | `65536` |
| `profile.target_size` | קומפרימנטו אלבו לעשות נתח בבתים. | `262144` |
| `profile.max_size` | קומפרימנטו מקסימו לעשות נתח בבתים. | `524288` |
| `profile.break_mask` | Mascara adaptativa usada pelo רולינג hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante do polinomio gear (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed usada עבור נגזרות של טבלת ציוד של 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Codigo multihash para digests por chunk. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest do bundle canonico de fixtures. | `13fa...c482` |
| `fixtures_root` | Diretorio relativo contendo os fixtures regenerados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed para amostragem PoR deterministica (`splitmix64`). | `0xfeedbeefcafebabe` (דוגמה) |

Os metadados devem aparecer tanto no documento de proposta quanto dentro dos fixtures gerados
para que o registro, o tooling de CLI e a automacao de governanca confirmem os valores sem
cruzamentos manuais. במקרה זה, הפעל את OS CLIs de chunk-store e manifest com
`--json-out=-` עבור שידורים של חישובים מתקדמים עבור מסמכים דה רוויזאו.

### רישום של CLI

- `sorafs_manifest_chunk_store --profile=<handle>` - ביצועים חוזרים של ה-chunk,
  לעכל לעשות מניפסט e בדיקות PoR com os parametros propostos.
- `sorafs_manifest_chunk_store --json-out=-` - שידור או קשר לחנות chunk para para
  stdout para comparacoes automatizadas.
- `sorafs_manifest_stub --chunker-profile=<handle>` - confirmar que manifests e planos CAR
  אבוטום או לטפל בכינויי canonico mais.
- `sorafs_manifest_stub --plan=-` - reenviar o `chunk_fetch_specs` קדמית
  אימות מקזז/מעכל את apos a mudanca.

רשום a saida dos comandos (מעכל, מעלה PoR, hashes de manifest) בהצעה
os revisores possam reproduzi-los literalmente.

## רשימת רשימת החלטות ותקינות1. **משחקי Regenerar**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Executar a suite de paridade** - `cargo test -p sorafs_chunker` e o harness diff
   חוצה שפות (`crates/sorafs_chunker/tests/vectors.rs`) devem ficar verdes com os
   נובוס מתקנים ללא lugar.
3. **הפעל מחדש גופים מטושטש/לחץ אחורי** - בצע `cargo fuzz list` e o harness de
   סטרימינג (`fuzz/sorafs_chunker`) contra os assets regenerados.
4. **אמת הוכחה לשליפה** - ביצוע
   `sorafs_manifest_chunk_store --por-sample=<n>` usando o perfil proposto e אישור
   que as raizes correspondem ao manifest de fixtures.
5. **Dry run de CI** - בצע `ci/check_sorafs_fixtures.sh` localmente; o תסריט
   deve ter sucesso com os novos fixtures e o `manifest_signatures.json` existente.
6. **Confirmacao זמן ריצה צולב** - הבטחת חיבורי מערכת ההפעלה Go/TS consumam o JSON
   regenerado e emitam limites e digests identicos.

תוצאות מסמכים של מערכות מידע ומערכות מידע על כלי עבודה WG
reexecuta-los sem adivinhacoes.

### Confirmacao de Manifest / PoR

Depois de regenerar fixtures, להפעיל או צנרת השלם דה מניפסט עבור garantir que
שיטות CAR e provas PoR מתמשכות עקביות:

```bash
# Validar metadados de chunk + PoR com o novo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Gerar manifest + CAR e capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reexecutar usando o plano de fetch salvo (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

גופי תחליפי או ארקיבו דה entrada por qualquer corpus representativo usado nos seus
(לדוגמה, o stream deterministico de 1 GiB) e anexe os digests resultantes a proposta.

## Modelo de Proposta

כמו הצעות סאו תת-מדידס como registros Norito `ChunkerProfileProposalV1` registrados em
`docs/source/sorafs/proposals/`. תבנית JSON תמונה או תבנית אספרדו
(substitua seus valores conforme necessario):


Forneca um relatorio Markdown correspondente (`determinism_report`) que capture a
saida dos comandos, digests de chunk e quaisquer desvios encontrados durante a validacao.

## Fluxo de governanca

1. **Submeter PR com proposta + fixtures.** Inclua os assets gerados, a proposta
   Norito e atualizacoes em `chunker_registry_data.rs`.
2. **Revisao do Tooling WG.** מבצע מחדש את רשימת הבדיקה של validacao e confirmam
   que a proposta segue as regras do registro (sem reutilizacao de id, determinismo satisfeito).
3. **Envelope do conselho.** Uma vez aprovado, membros do conselho assinam o digest da
   proposta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) e anexam suas
   assinaturas ao envelope do perfil armazenado junto aos גופי.
4. **Publicacao do registro.** או מיזוג אוטואליזה או רישום, מסמכים ותקנים. O CLI
   default permanece no perfil anterior ate que a governanca להכריז על migracao pronta.
5. **Rastreamento de deprecacao.** Apos a janela de migracao, atualize o registro para

## Dicas de autoria

- Prefira limites pares de potencia de dois para minimizar comportamento de chunking em bordas.
- Evite mudar o codigo multihash sem coordenar consumidores de manifest e gateway; כולל אומה
  not operation quando fizer isso.
- Mantenha as seeds da tabela gear legiveis para humanos, mas globalmente unicas para simplificar auditorias.
- Armazene artefatos de benchmarking (לדוגמה, comparacoes de throughput) em
  `docs/source/sorafs/reports/` עבור התייחסות עתידית.עבור פעולות צפויות במהלך ההשקה, התייעצות או פנקס ההגירה
(`docs/source/sorafs/migration_ledger.md`). Para regras de conformidade em runtime, veja
`docs/source/sorafs/chunker_conformance.md`.