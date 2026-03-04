---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidade com chunker
título: Guia de conformidade do chunker SoraFS
sidebar_label: Conformidade do Chunker
description: Fixtures e SDKs میں perfil determinístico do chunker SF1 کو برقرار رکھنے کے لیے requisitos e fluxos de trabalho
---

:::nota مستند ماخذ
:::

یہ fluxo de trabalho de regeneração, política de assinatura, etapas de verificação بھی documento کرتی ہے تاکہ SDKs میں sincronização de consumidores de dispositivos میں رہیں۔

## Perfil canônico

- Semente de entrada (hex): `0000000000dec0ded`
- Tamanho alvo: 262.144 bytes (256 KiB)
- Tamanho mínimo: 65536 bytes (64 KiB)
- Tamanho máximo: 524288 bytes (512 KiB)
- Polinômio rolante: `0x3DA3358B4DC173`
- Semente da mesa de engrenagens: `sorafs-v1-gear`
Máscara de quebra: `0x0000FFFF`

Implementação de referência: `sorafs_chunker::chunk_bytes_with_digests_profile`.
کسی بھی Aceleração SIMD کو یکساں limites اور digests پیدا کرنے چاہئیں۔

## Pacote de luminárias

Acessórios `cargo run --locked -p sorafs_chunker --bin export_vectors` کو regenerar
O valor do cartão `fixtures/sorafs_chunker/` é o seguinte:

- `sf1_profile_v1.{json,rs,ts,go}` — Rust, TypeScript, e consumidores Go کے لیے limites de pedaços canônicos۔ ہر فائل
  آتے ہیں (مثلاً `sorafs.sf1@1.0.0`, پھر `sorafs.sf1@1.0.0`)۔ یہ pedido
  `ensure_charter_compliance` کے ذریعے impor ہوتی ہے اور اسے تبدیل نہیں کیا جا سکتا۔
- `manifest_blake3.json` — Manifesto verificado pelo BLAKE3 جو ہر fixture فائل کو cover کرتا ہے۔
- `manifest_signatures.json` — resumo do manifesto پر assinaturas do conselho (Ed25519)۔
- `sf1_profile_v1_backpressure.json` e `fuzz/` کے اندر corpora brutos —
  cenários de streaming determinísticos e testes de contrapressão de chunker میں استعمال ہوتے ہیں۔

### Política de assinatura

Regeneração de luminária **لازم** طور پر assinatura válida do conselho شامل کرے۔ saída não assinada do gerador کو rejeitar کرتا ہے جب تک `--allow-unsigned` واضح طور پر نہ دیا جائے (صرف مقامی تجربات کے لیے)۔ Envelopes de assinatura somente anexados ہوتے ہیں اور signatário کے لحاظ سے desduplicar ہوتے ہیں۔

Assinatura do Conselho شامل کرنے کے لیے:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificação

CI helper `ci/check_sorafs_fixtures.sh` gerador کو `--locked` کے ساتھ دوبارہ چلاتا ہے۔
اگر fixtures میں drift ہو یا assinaturas faltando ہوں تو falha no trabalho ہو جاتا ہے۔ O script é
fluxos de trabalho noturnos میں اور alterações de luminárias enviadas کرنے سے پہلے استعمال کریں۔

Etapas de verificação manual:

1. `cargo test -p sorafs_chunker` چلائیں۔
2. `ci/check_sorafs_fixtures.sh` Placa de vídeo
3. تصدیق کریں کہ `git status -- fixtures/sorafs_chunker` صاف ہے۔

## Manual de atualização

O perfil do chunker propõe کرتے وقت یا SF1 اپڈیٹ کرتے وقت:

یہ بھی دیکھیں: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) تاکہ
requisitos de metadados, modelos de propostas e listas de verificação de validação.

1. Parâmetros نئے کے ساتھ `ChunkProfileUpgradeProposalV1` (RFC SF-1 دیکھیں) تیار کریں۔
2. `export_vectors` کے ذریعے fixtures regeneram کریں اور نیا manifest digest ریکارڈ کریں۔
3. مطلوبہ quórum do conselho کے ساتھ sinal de manifesto کریں۔ Assinaturas de تمام
   `manifest_signatures.json` میں anexar ہونی چاہئیں۔
4. Atribuir dispositivos SDK (Rust/Go/TS) اپڈیٹ کریں اور paridade de tempo de execução cruzado یقینی بنائیں۔
5. Parâmetros بدلیں e corpora fuzz regenerados کریں۔
6. اس گائیڈ میں نیا identificador de perfil, sementes, اور digest اپڈیٹ کریں۔
7. Faça testes e atualizações do roteiro e envie-osاگر اس عمل کے بغیر limites de pedaços یا digests تبدیل کیے جائیں تو وہ inválido ہیں اور merge نہیں ہونے چاہئیں۔