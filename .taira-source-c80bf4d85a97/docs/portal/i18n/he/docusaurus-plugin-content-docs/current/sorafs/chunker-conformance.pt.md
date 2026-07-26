---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-conformance
כותרת: Guia de conformidade do chunker da SoraFS
sidebar_label: Conformidade de chunker
תיאור: דרישות וזרימות לשמירה על הגדרות קבועות של chunker SF1 עם מתקנים ו-SDKs.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/chunker_conformance.md`. Mantenha ambas as copias sincronizadas.
:::

Este guia codifica os requisitos que toda implementacao deve seguir para permanecer
קומפטיבי com o perfil deterministico de chunker da SoraFS (SF1). אל טמבם
documenta o fluxo de regeneracao, a politica de assinaturas e os passos de verificacao para que
OS consumidores de fixtures nos SDKs Fiquem Sincronizados.

## פרופיל canonico

- ידית לעשות פרפיל: `sorafs.sf1@1.0.0`
- Seed de entrada (hex): `0000000000dec0ded`
- Tamanho alvo: 262144 בייטים (256 KiB)
- Tamanho minimo: 65536 בתים (64 KiB)
- Tamanho maximo: 524288 בייטים (512 KiB)
- Polinomio de rolling: `0x3DA3358B4DC173`
- Seed da tabela ציוד: `sorafs-v1-gear`
- מסיכת שבירה: `0x0000FFFF`

יישום רפרנס: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Qualquer aceleracao SIMD deve produzir limites e digests identicos.

## חבילת מתקנים

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera as
מתקנים e emite os seguintes arquivos em `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` - מגביל את ה-canonicos de chunk לצריכה
  Rust, TypeScript ו-Go. Cada arquivo anuncia o handle canonico como a primeira
  entrada em `profile_aliases`, seguido de quaisquer aliases alternativos (לדוגמה,
  `sorafs.sf1@1.0.0`, depois `sorafs.sf1@1.0.0`). A orderem e posta por
  `ensure_charter_compliance` e NAO DEVE ser alterada.
- `manifest_blake3.json` - אימות מניפסט על ידי BLAKE3 cobrindo cada arquivo de fixtures.
- `manifest_signatures.json` - assinaturas do conselho (Ed25519) sobre o digest do manifest.
- `sf1_profile_v1_backpressure.json` e corpora brutos dentro de `fuzz/` -
  תרחישים קובעים של סטרימינג בארה"ב לבדיקות של לחץ אחורי לעשות צ'אנקר.

### Politica de assinaturas

מכשירי **מפתחים מחדשים** כוללים את התאמת הקונסולות. הו גרדור
rejeita saida sem assinatura a menos que `--allow-unsigned` seja passado explicitamente (destinado
apenas para experimentacao local). Os envelopes de assinatura sao הוספה בלבד ה
sao deduplicados por signatario.

למידע נוסף:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificacao

O helper de CI `ci/check_sorafs_fixtures.sh` reexecuta o gerador com
`--locked`. Se fixtures divergirem או assinaturas faltarem, o job falha. השתמש
es script עבור זרימות עבודה ותרגילים.

מדריך מעברים דה verificacao:

1. הפעל את `cargo test -p sorafs_chunker`.
2. בצע את `ci/check_sorafs_fixtures.sh` localmente.
3. אשר que `git status -- fixtures/sorafs_chunker` esta limpo.

## Playbook de upgrade

Ao propor um novo perfil de chunker או atualizar o SF1:

ויה טמבם: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
requisitos de metadados, templates de proposta e checklists de validacao.1. Redija um `ChunkProfileUpgradeProposalV1` (veja RFC SF-1) עם פרמטרים חדשים.
2. Regenere fixtures דרך `export_vectors` e registre או novo digest do manifest.
3. Assine o manifest com o quorum do conselho exigido. Todas as assinaturas devem ser
   anexadas a `manifest_signatures.json`.
4. התאמת כמתקנים של SDK (Rust/Go/TS) ו-garanta paridade cross-runtime.
5. Regenere corpora fuzz se os parametros mudarem.
6. להטמיע את זה e guia com o novo handle de perfil, seeds e digest.
7. Envie a mudanca junto com testes atualizados e atualizacoes do מפת דרכים.

Mudancas que afetem limites de chunk או digests sem seguir este processo
sao invalidas e nao devem ser mergeadas.