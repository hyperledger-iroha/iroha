---
lang: pt
direction: ltr
source: docs/portal/docs/reference/norito-codec.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مرجع ترميز Norito

Norito é um recurso de substituição em Iroha. A conexão on-wire e a carga útil da API e da API são configuradas com Norito para serem executadas sem problemas Você pode fazer isso sem problemas. Verifique o valor do arquivo e verifique o valor do arquivo em `norito.md`.

## البنية الاساسية

| المكون | الغرض | المصدر |
| --- | --- | --- |
| **الرأس** | يؤطر payloads مع magic/version/schema hash و CRC64 والطول وعلامة الضغط؛ v1 contém `VERSION_MINOR = 0x00` e tem sinalizadores de cabeçalho como o `0x00`. | `norito::header` — راجع `norito.md` ("Cabeçalho e sinalizadores", جذر المستودع) |
| **Carga útil بدون رأس** | ترميز قيم حتمي يستخدم para hashing/المقارنة. O on-wire é uma opção sem fio Não há nada que você possa fazer. | `norito::codec::{Encode, Decode}` |
| **الضغط** | Zstd اختياري (e GPU تجريبي) يتم اختياره عبر بايت الضغط في الرأس. | `norito.md`, “Negociação de compactação” |

Os sinalizadores estão no layout (packed-struct, pack-seq, field bitset, compact lengths) são encontrados em `norito::header::flags`. Use flags V1 `0x00` para configurar flags يتم رفض البتات غير المعروفة. Use o `norito::header::Flags` para obter informações sobre o produto e o produto.

## دعم deriva

O `norito_derive` contém `Encode`, `Decode`, `IntoSchema` e JSON. Como fazer:

- المشتقات تولد مسارات AoS e embalado; v1 é definido como AoS افتراضيا (flags `0x00`) para que os header flags sejam compactados. O modelo é `crates/norito_derive/src/derive_struct.rs`.
- الميزات المؤثرة على التخطيط (`packed-struct`, `packed-seq`, `compact-len`) é opt-in عبر header flags ويجب ترميزها/فك ترميزها بشكل متسق عبر pares.
- O JSON (`norito::json`) é o JSON que é gerado pelo Norito para a API da API. Use `norito::json::{to_json_pretty, from_json}` — e use `serde_json`.

## Multicodec e código de barras

O Norito é um multicodec do `norito::multicodec`. O valor de hashes (hashes, a carga útil e a carga útil) é encontrado em `multicodec.md`. A melhor maneira de fazer isso:

1. Selecione `norito::multicodec::registry`.
2. Verifique o código em `multicodec.md`.
3. Verifique as ligações downstream (Python/Java).

## اعادة توليد documentos e luminárias

مع استضافة البوابة حاليا لملخص وصفي, استخدم مصادر Markdown الاصلية كمصدر للحقيقة:

**Especificações**: `norito.md`
- **Multicodec**: `multicodec.md`
- **Referências**: `crates/norito/benches/`
- **Testes de ouro**: `crates/norito/tests/`

Para definir o Docusaurus, você pode usar o Docusaurus para sincronizar (como o `docs/portal/scripts/`) يسحب البيانات من هذه الملفات. Não se preocupe, você pode fazer isso com mais frequência.