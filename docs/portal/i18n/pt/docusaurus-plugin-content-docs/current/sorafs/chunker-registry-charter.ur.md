---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-charter
título: Carta de registro do chunker SoraFS
sidebar_label: Carta de registro do Chunker
descrição: Envios de perfis Chunker e aprovações کے لیے estatuto de governança۔
---

:::nota مستند ماخذ
:::

# Carta de governança de registro de chunker SoraFS

> **Ratificado:** 2025-10-29 pelo Painel de Infraestrutura do Parlamento Sora (ver
> `docs/source/sorafs/council_minutes_2025-10-29.md`). کسی بھی ترمیم کے لیے باضابطہ governança e ووٹ درکار ہے؛
> implementação ٹیموں کو اس دستاویز کو اس وقت تک normativo سمجھنا ہوگا جب تک کوئی نئی charter منظور نہ ہو۔

یہ charter SoraFS chunker registro کو evoluir کرنے کے لیے processo اور funções definir کرتی ہے۔
یہ [Guia de criação de perfil Chunker](./chunker-profile-authoring.md) کی تکمیل کرتی ہے اور یہ بیان کرتی ہے کہ نئے
perfis کیسے propor, revisar, ratificar اور بالآخر obsoleto ہوتے ہیں۔

## Escopo

یہ charter `sorafs_manifest::chunker_registry` کی ہر entrada پر لاگو ہے اور
ہر اس ferramentas پر بھی جو registro استعمال کرتا ہے (CLI manifesto, CLI de anúncio de provedor,
SDK). یہ alias اور lidar com invariantes impor کرتی ہے جنہیں
`chunker_registry::ensure_charter_compliance()` چیک کرتا ہے:

- IDs de perfil مثبت inteiros ہوتے ہیں جو monotônico طور پر بڑھتے ہیں۔
- Cabo canônico `namespace.name@semver` **لازم** ہے کہ `profile_aliases` میں پہلی
- Corte de strings de alias کی جاتی ہیں، único ہوتی ہیں, اور دوسری entradas کے identificadores canônicos سے colidir نہیں کرتیں۔

## Funções

- **Autor(es)** – proposta تیار کرتے ہیں، fixtures regeneram کرتے ہیں, اور evidência de determinismo جمع کرتے ہیں۔
- **Grupo de Trabalho de Ferramentas (TWG)** – listas de verificação publicadas کے ذریعے proposta validada کرتا ہے اور invariantes de registro برقرار رکھتا ہے۔
- **Conselho de Governança (GC)** – Relatório do TWG کا revisão کرتا ہے، envelope da proposta پر assinar کرتا ہے، اور cronogramas de publicação/descontinuação aprovar کرتا ہے۔
- **Equipe de armazenamento** – implementação de registro, manutenção کرتا ہے اور, atualizações de documentação, publicação کرتا ہے۔

## Fluxo de trabalho do ciclo de vida

1. **Envio de proposta**
   - Guia de autoria do autor کی lista de verificação de validação چلاتا ہے اور
     `docs/source/sorafs/proposals/` é definido como `ChunkerProfileProposalV1` JSON
   - Saída CLI شامل کریں:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Luminárias, proposta, relatório de determinismo, e atualizações de registro پر مشتمل PR enviar کریں۔

2. **Revisão de ferramentas (TWG)**
   - Lista de verificação de validação دوبارہ چلائیں (fixtures, fuzz, manifesto/pipeline PoR).
   - `cargo test -p sorafs_car --chunker-registry` چلائیں اور یقینی بنائیں کہ
     `ensure_charter_compliance()` entrada de entrada کے ساتھ پاس ہو۔
   - Comportamento CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) کی توثیق کریں کہ یہ aliases atualizados اور identificadores دکھاتا ہے۔
   - Descobertas e status de aprovação/reprovação پر مشتمل مختصر رپورٹ تیار یں۔

3. **Aprovação do Conselho (GC)**
   - Relatório do TWG e metadados da proposta e revisão
   - Resumo da proposta (`blake3("sorafs-chunker-profile-v1" || bytes)`) پر sinal کریں اور
     assinaturas کو envelope do conselho میں شامل کریں جو luminárias کے ساتھ رکھا جاتا ہے۔
   - Resultado da votação کو ata de governança میں ریکارڈ کریں۔4. **Publicação**
   - PR merge کریں، اور اپڈیٹ کریں:
     -`sorafs_manifest::chunker_registry_data`.
     - Documentação (`chunker_registry.md`, guias de autoria/conformidade).
     - Luminárias e relatórios de determinismo.
   - Operadores اور equipes SDK کو نئے perfil اور implementação planejada کے بارے میں اطلاع دیں۔

5. **Descontinuação/Descontinuação**
   - جو propostas موجودہ perfil کو substituir کرتی ہیں ان میں janela de publicação dupla
     (períodos de carência) اور plano de atualização شامل ہونا چاہیے۔
     O registro de migração e a atualização do registro

6. **Mudanças emergenciais**
   - Remoção یا hotfixes کے لیے votação do conselho اور اکثریتی aprovação درکار ہے۔
   - Documento de etapas de mitigação de risco do TWG کرنے اور atualização do log de incidentes کرنے ہوں گے۔

## Expectativas de ferramentas

- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` expor o seguinte:
  - Inspeção de registro کے لیے `--list-profiles`.
  - Promoção de perfil کرتے وقت bloco de metadados canônicos بنانے کے لیے `--promote-profile=<handle>`.
  - Relatórios کو stdout پر stream کرنے کے لیے `--json-out=-`, تاکہ logs de revisão reproduzíveis ممکن ہوں۔
- Binários relevantes `ensure_charter_compliance()` کے startup پر چلایا جاتا ہے
  (`manifest_chunk_store`, `provider_advert_stub`). Testes de CI falham e falham
  Carta de entradas کی خلاف ورزی کریں۔

## Manutenção de registros

- Relatórios de determinismo تمام کو `docs/source/sorafs/reports/` میں محفوظ کریں۔
- Chunker فیصلوں کا حوالہ دینے والی atas do conselho
  `docs/source/sorafs/migration_ledger.md` میں موجود ہیں۔
- ہر بڑے alteração de registro کے بعد `roadmap.md` ou `status.md` اپڈیٹ کریں۔

## Referências

- Guia de autoria: [Guia de autoria de perfil Chunker](./chunker-profile-authoring.md)
Lista de verificação de conformidade: `docs/source/sorafs/chunker_conformance.md`
- Referência do registro: [Registro de perfil do Chunker] (./chunker-registry.md)