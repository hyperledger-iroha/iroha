---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Absorção de acumulação de capacidade SF-2c

Data: 2026-03-21

## سکوپ

یہ رپورٹ SF-2c روڈمیپ ٹریک کے تحت مانگے گئے SoraFS acumulação de capacidade e pagamento کے testes de absorção determinísticos ریکارڈ کرتی ہے۔

- **30 dias de imersão multi-provedor:**
  `capacity_fee_ledger_30_day_soak_deterministic` کے ذریعے
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں چلایا جاتا ہے۔
  aproveitar پانچ provedores بناتا ہے، 30 liquidação ونڈوز کا احاطہ کرتا ہے، اور
  تصدیق کرتا ہے کہ totais do razão ایک آزادانہ طور پر حساب شدہ projeção de referência سے ملتے ہیں۔
  یہ ٹیسٹ Blake3 digest (`capacity_soak_digest=...`) خارج کرتا ہے تاکہ CI canonical snapshot کو
  capturar e diff کر سکے۔
- **Penalidades por subentrega:**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (اسی فائل میں) نافذ کیا جاتا ہے۔ ٹیسٹ تصدیق کرتا ہے کہ atinge limites, cooldowns, barras colaterais
  Contadores contábeis determinísticos رہتے ہیں۔

## Execução

absorver validações لوکل طور پر چلائیں:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

یہ ٹیسٹ ایک عام لیپ ٹاپ پر ایک سیکنڈ سے کم وقت میں مکمل ہو جاتے ہیں اور بیرونی luminárias کی ضرورت نہیں ہوتی۔

## Observabilidade

Torii اب instantâneos de crédito do provedor کو livros de taxas کے ساتھ ظاہر کرتا ہے تاکہ painéis کم saldos اور penalidades پر portão کر سکیں:

- REST: `GET /v1/sorafs/capacity/state` `credit_ledger[*]` entradas واپس کرتا ہے جو teste de imersão میں verificar ہونے والے
  campos contábeis کی عکاسی کرتے ہیں۔ دیکھیں
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importação Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` contadores de golpes exportados, totais de penalidades,
  اور garantia garantida کو parcela کرتا ہے تاکہ de plantão ٹیم absorver linhas de base کو ambientes ao vivo سے موازنہ کر سکے۔

## Acompanhamento

- CI میں ہفتہ وار gate executa شیڈول کریں تاکہ teste de imersão دوبارہ چل سکے (camada de fumaça).
- جب exportações de telemetria de produção ao vivo ہوں تو placa Grafana میں alvos de raspagem Torii شامل کریں۔