---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidade com chunker
título: Um pedaço de pedaço em SoraFS
sidebar_label: Nome do chunker
description: Você pode usar o chunker do SF1 para usar fixtures e SDKs.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/chunker_conformance.md`. Certifique-se de que o produto esteja mais limpo do que o normal.
:::

يوثق هذا الدليل المتطلبات التي يجب على كل تطبيق اتباعها للبقاء متوافقاً مع ملف chunker الحتمي Em SoraFS (SF1).
Você pode usar jogos e dispositivos móveis com SDKs متزامنين.

## الملف المعتمد

- Nome do modelo: `sorafs.sf1@1.0.0` (`sorafs.sf1@1.0.0`)
- بذرة الإدخال (hex): `0000000000dec0ded`
- Número de bits: 262144 bytes (256 KiB)
- Número de bits: 65536 bytes (64 KiB)
- Número de bits: 524288 bytes (512 KiB)
- Código de identificação: `0x3DA3358B4DC173`
Engrenagem de engrenagem: `sorafs-v1-gear`
- Nome do usuário: `0x0000FFFF`

Nome do usuário: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Não há nenhum resumo do SIMD e resumos.

## حزمة jogos

`cargo run --locked -p sorafs_chunker --bin export_vectors` `cargo run --locked -p sorafs_chunker --bin export_vectors`
fixtures ويصدر الملفات التالية ضمن `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — Um pedaço de pedaço de Rust, TypeScript e Go.
  Não há nenhum problema em `profile_aliases`, mas não há necessidade de fazer nada (como
  `sorafs.sf1@1.0.0` ou `sorafs.sf1@1.0.0`). يتم فرض الترتيب بواسطة
  `ensure_charter_compliance` é um problema.
- `manifest_blake3.json` — manifesto do arquivo de fixtures.
- `manifest_signatures.json` — توقيعات المجلس (Ed25519) para digerir o arquivo do manifesto.
- `sf1_profile_v1_backpressure.json` e corpora do corpo `fuzz/` —
  Você pode usar um pedaço de contrapressão para um pedaço de contrapressão.

### سياسة التوقيع

يجب أن تشمل إعادة توليد fixtures توقيعاً صالحاً من المجلس. يرفض المولد
O código de barras `--allow-unsigned` está disponível para download (recomendado).
للتجارب المحلية فقط). أظرف التوقيع somente acréscimo ويتم إزالة التكرارات حسب الموقّع.

Veja o que acontece no site:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

Use o CI `ci/check_sorafs_fixtures.sh` para obter mais informações
`--locked`. إذا انحرفت fixtures, أو غابت التواقيع, تفشل المهمة. استخدم
Existem fluxos de trabalho em fluxos de trabalho e dispositivos elétricos.

خطوات التحقق اليدوية:

1. Instale `cargo test -p sorafs_chunker`.
2. Use `ci/check_sorafs_fixtures.sh`.
3. Verifique o `git status -- fixtures/sorafs_chunker`.

## دليل الترقية

عند اقتراح ملف chunker جديد أو تحديث SF1:

A opção: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) é uma opção
البيانات الوصفية وقوالب المقترح وقوائم التحقق.

1. O código `ChunkProfileUpgradeProposalV1` (RFC SF-1) está disponível.
2. Verifique fixtures em `export_vectors` e digerir o manifesto.
3. وقّع الـ manifest بحصة المجلس المطلوبة. O problema é `manifest_signatures.json`.
4. Verifique os fixtures dos SDKs (Rust/Go/TS) e instale-os.
5. Deixe os corpora fuzz em ordem decrescente.
6. حدّث هذا الدليل بالمقبض الجديد للملف والبذور e Digest.
7. قدّم التغيير مع الاختبارات المحدثة وتحديثات roteiro.

التغييرات التي تؤثر على حدود الـ chunk, أو الـ digests دون اتباع هذه العملية
Não há nada que você possa fazer.