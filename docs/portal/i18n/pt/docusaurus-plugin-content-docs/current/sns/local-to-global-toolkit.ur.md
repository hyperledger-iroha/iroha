---
lang: pt
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Local -> Global ایڈریس ٹول کٹ

یہ صفحہ `docs/source/sns/local_to_global_toolkit.md` کا عکاس ہے۔ یہ roadmap آئٹم **ADDR-5c** کے لیے درکار CLI helpers e runbooks اکٹھے کرتا ہے۔

## جائزہ

- `scripts/address_local_toolkit.sh` `iroha` CLI کو wrap کرتا ہے تاکہ یہ پیدا کرے:
  - `audit.json` -- `iroha tools address audit --format json` کا saída estruturada۔
  - `normalized.txt` -- ہر Seletor de domínio local کے لیے IH58 (ترجیحی) / compactado (`sora`, segundo melhor) literais۔
- اس اسکرپٹ کو painel de ingestão de endereço (`dashboards/grafana/address_ingest.json`)
  اور Alertmanager rules (`dashboards/alerts/address_ingest_rules.yml`) کے ساتھ استعمال کریں تاکہ
  Corte local-8 / local-12 Local-8 ou Local-12 painéis de colisão
  Alertas `AddressLocal8Resurgence`, `AddressLocal12Collision`, e `AddressInvalidRatioSlo`
  کو manifesto تبدیلیاں promover کرنے سے پہلے دیکھیں۔
- UX para resposta a incidentes کے لیے [Diretrizes de exibição de endereço](address-display-guidelines.md) اور
  [Runbook de manifesto de endereço](../../../source/runbooks/address_manifest_ops.md) کو دیکھیں۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Informações:

- `--format compressed` IH58 کے بجائے Saída `sora...` کے لیے۔
- `domainless output (default)` تاکہ literais simples نکلیں۔
- Etapa de conversão `--audit-only` چھوڑنے کے لیے۔
- `--allow-errors` تاکہ linhas malformadas پر بھی scan جاری رہے (comportamento CLI جیسا)۔

اسکرپٹ رن کے آخر میں caminhos de artefato لکھتا ہے۔ دونوں فائلیں
ticket de gerenciamento de alterações کے ساتھ منسلک کریں اور Grafana captura de tela بھی شامل کریں جو
>=30 دن تک صفر Detecções locais-8 اور صفر Colisões locais-12 دکھائے۔

## CI انضمام

1. اسکرپٹ کو trabalho dedicado میں چلائیں اور saídas اپ لوڈ کریں۔
2. جب `audit.json` Seletores locais رپورٹ کرے (`domain.kind = local12`) تو mescla روک دیں۔
   padrão `true` پر رکھیں (صرف dev/test میں regressões کی تشخیص کے وقت `false` کریں) اور
   `iroha tools address normalize` کو CI میں شامل کریں تاکہ
   produção de regressões

Listas de verificação de evidências e trechos de notas de lançamento کے لیے سورس دستاویز
Qual é o corte de corte que você precisa e qual é o valor do seu cartão de crédito?