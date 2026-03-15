---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c Almacenamiento de acumulación de capacidad رپورٹ

Actualizado: 2026-03-21

## اسکوپ

یہ رپورٹ SF-2c روڈمیپ ٹریک کے تحت مانگے گئے SoraFS acumulación de capacidad اور pago کے pruebas de remojo deterministas ریکارڈ کرتی ہے۔

- **30 días de inmersión en múltiples proveedores:**
  `capacity_fee_ledger_30_day_soak_deterministic` کے ذریعے
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں چلایا جاتا ہے۔
  aprovechar پانچ proveedores بناتا ہے، 30 liquidación ونڈوز کا احاطہ کرتا ہے، اور
  تصدیق کرتا ہے کہ totales del libro mayor ایک آزادانہ طور پر حساب شدہ proyección de referencia سے ملتے ہیں۔
  یہ ٹیسٹ Resumen de Blake3 (`capacity_soak_digest=...`) خارج کرتا ہے تاکہ Instantánea canónica de CI کو
  capturar اور diff کر سکے۔
- **Sanciones por entrega insuficiente:**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (اسی فائل میں) نافذ کیا جاتا ہے۔ ٹیسٹ تصدیق کرتا ہے کہ alcanza umbrales, tiempos de reutilización, cortes colaterales
  اور contadores deterministas رہتے ہیں۔

## Ejecución

validaciones de remojo لوکل طور پر چلائیں:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

یہ ٹیسٹ ایک عام لیپ ٹاپ پر ایک سیکنڈ سے کم وقت میں مکمل ہو جاتے ہیں اور بیرونی accesorios کی ضرورت نہیں ہوتی۔

## Observabilidad

Torii اب instantáneas de crédito del proveedor کو libros de tarifas کے ساتھ ظاہر کرتا ہے تاکہ paneles کم saldos اور penalizaciones پر puerta کر سکیں:- RESTO: `GET /v2/sorafs/capacity/state` `credit_ledger[*]` entradas واپس کرتا ہے جو prueba de remojo میں verificar ہونے والے
  campos del libro mayor کی عکاسی کرتے ہیں۔ دیکھیں
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importación Grafana: contadores de strikes exportados `dashboards/grafana/sorafs_capacity_penalties.json`, totales de penalización,
  اور garantía garantizada کو trama کرتا ہے تاکہ de guardia ٹیم líneas de base de remojo کو entornos en vivo سے موازنہ کر سکے۔

## Seguimiento

- CI میں ہفتہ وار gate ejecuta شیڈول کریں تاکہ prueba de remojo دوبارہ چل سکے (nivel de humo).
- Exportaciones de telemetría de producción en vivo ہوں تو Tablero Grafana میں Objetivos de raspado Torii شامل کریں۔