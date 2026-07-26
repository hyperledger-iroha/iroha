---
lang: pt
direction: ltr
source: docs/portal/docs/reference/norito-codec.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Referência do codec Norito

Norito Iroha کی camada de serialização canônica ہے۔ ہر mensagem on-wire, carga útil em disco, e API de componente cruzado Norito استعمال کرتا ہے تاکہ nós مختلف hardware پر بھی ایک جیسے bytes پر متفق رہیں۔ یہ صفحہ اہم حصے خلاصہ کرتا ہے اور مکمل especificação کے لئے `norito.md` کی طرف اشارہ کرتا ہے۔

## Layout principal

| Componente | Finalidade | Fonte |
| --- | --- | --- |
| **Cabeçalho** | hash mágico / versão / esquema, CRC64, comprimento, tag de compactação e cargas úteis کو فریم کرتا ہے؛ v1 é `VERSION_MINOR = 0x00` ضروری ہے اور sinalizadores de cabeçalho کو máscara suportada کے مقابل validar کیا جاتا ہے (padrão `0x00`). | `norito::header` — `norito.md` ("Cabeçalho e sinalizadores", raiz do repositório) دیکھیں |
| **Carga útil nua** | hashing / موازنہ کیلئے codificação de valor determinística۔ Transporte on-wire ہمیشہ cabeçalho استعمال کرتا ہے؛ bytes nus | `norito::codec::{Encode, Decode}` |
| **Compressão** | Zstd opcional (aceleração experimental de GPU) e cabeçalho کے byte de compactação کے ذریعے منتخب ہوتی ہے۔ | `norito.md`, “Negociação de compactação” |

registro de sinalizador de layout (estrutura compactada, seq compactada, conjunto de bits de campo, comprimentos compactos) `norito::header::flags` میں ہے۔ Padrões V1 کے طور پر sinalizadores `0x00` استعمال کرتا ہے مگر máscara suportada کے اندر sinalizadores de cabeçalho explícitos قبول کرتا ہے؛ bits desconhecidos `norito::header::Flags` Inspeção de segurança اور مستقبل کی versões کیلئے رکھا جاتا ہے۔

## Obtenha suporte

`norito_derive` `Encode`, `Decode`, `IntoSchema` O auxiliar JSON deriva فراہم کرتا ہے۔ Convenções:

- Deriva caminhos de código compactados AoS e دونوں بناتے ہیں؛ Layout AoS v1 (sinalizadores `0x00`) کو padrão رکھتا ہے جب تک cabeçalho sinalizadores variantes compactadas کو opt-in نہ کریں۔ Implementação `crates/norito_derive/src/derive_struct.rs` میں ہے۔
- Layout پر اثر انداز ہونے والی recursos (`packed-struct`, `packed-seq`, `compact-len`) sinalizadores de cabeçalho کے ذریعے opt-in ہیں اور peers کے درمیان codificação/decodificação consistente ہونی چاہئیں۔
- Ajudantes JSON (`norito::json`) APIs abertas کیلئے JSON determinístico apoiado por Norito فراہم کرتے ہیں۔ `norito::json::{to_json_pretty, from_json}` کریں — کبھی `serde_json` نہیں۔

## Multicodec e tabelas de identificadores

Norito اپنی atribuições multicodec `norito::multicodec` میں رکھتا ہے۔ Tabela de referência (hashes, tipos de chave, descritores de carga útil) raiz do repositório کے `multicodec.md` میں برقرار رکھی جاتی ہے۔ Seu identificador é:

1. `norito::multicodec::registry` اپڈیٹ کریں۔
2. `multicodec.md` Mesa de mesa بڑھائیں۔
3. Mapear ligações downstream (Python/Java)

## Docs اور fixtures کو regenerate کرنا

جب پورٹل اس وقت host de resumo em prosa کر رہا ہے, fontes Markdown upstream e fonte da verdade رکھیں:

**Especificações**: `norito.md`
- **Tabela multicodec**: `multicodec.md`
- **Referências**: `crates/norito/benches/`
- **Testes de ouro**: `crates/norito/tests/`جب Docusaurus automação ao vivo ہو جائے, پورٹل ایک script de sincronização کے ذریعے اپڈیٹ ہوگا (جسے `docs/portal/scripts/` میں faixa کیا گیا ہے) جو ان arquivos سے dados کھینچتا ہے۔ تب تک، spec میں تبدیلی کے ساتھ اس صفحہ کو دستی طور پر alinhar رکھیں۔