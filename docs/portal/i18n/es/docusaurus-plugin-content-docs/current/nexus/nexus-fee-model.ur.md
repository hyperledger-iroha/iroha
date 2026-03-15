---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-tarifa-nexus
título: Nexus فیس ماڈل اپ ڈیٹس
descripción: `docs/source/nexus_fee_model.md` کا آئینہ، جو carriles recibos de liquidación اور superficies de reconciliación کی دستاویز کرتا ہے۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_fee_model.md` کی عکاسی کرتا ہے۔ جاپانی، عبرانی، ہسپانوی، پرتگالی، فرانسیسی، روسی، عربی اور اردو ترجمے migrar ہونے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Nexus فیس ماڈل اپ ڈیٹس

یکساں enrutador de liquidación اب ہر carril کے لئے recibos deterministas محفوظ کرتا ہے تاکہ آپریٹرز débitos de gas کو Nexus فیس ماڈل کے مطابق reconciliar کر سکیں۔- enrutador کی مکمل arquitectura, política de búfer, matriz de telemetría اور secuenciación de implementación کے لئے `docs/settlement-router.md` دیکھیں۔ یہ گائیڈ وضاحت کرتا ہے کہ یہاں درج parámetros کیسے Hoja de ruta de NX-3 entregable سے جڑتے ہیں اور SRe کو producción میں enrutador کی نگرانی کیسے کرنی چاہئے۔
- Configuración del activo de gas (`pipeline.gas.units_per_gas`) en formato decimal `twap_local_per_xor`, `liquidity_profile` (`tier1`, `tier2`, y `tier3`) `volatility_class` (`stable`, `elevated`, `dislocated`) Negro یہ enrutador de asentamiento de banderas کو feed ہوتے ہیں تاکہ حاصل ہونے والی cotización XOR, TWAP canónico اور carril کے nivel de corte de pelo سے میل کھائے۔
- ہر pago de gas والی transacción ایک `LaneSettlementReceipt` ریکارڈ کرتی ہے۔ ہر recibo de llamada کی فراہم کردہ identificador de fuente, micro-monto local, فوری واجب الادا XOR, corte de pelo کے بعد esperado XOR, حاصل شدہ variación (`xor_variance_micro`) اور marca de tiempo del bloque (milisegundos) محفوظ کرتا ہے۔
- Bloquear recibos de ejecución کو carril/espacio de datos کے حساب سے agregado کرتا ہے اور انہیں `/v1/sumeragi/status` میں `lane_settlement_commitments` کے ذریعے شائع کرتا ہے۔ totales میں `total_local_micro`, `total_xor_due_micro`, اور `total_xor_after_haircut_micro` شامل ہوتے ہیں جو block پر جمع کر کے conciliación nocturna exportaciones کے لئے فراہم ہوتے ہیں۔- ایک نیا `total_xor_variance_micro` contador یہ pista کرتا ہے کہ کتنا margen de seguridad استعمال ہوا (debido a XOR اور expectativa posterior al corte de pelo کے درمیان فرق), اور Parámetros de conversión deterministas `swap_metadata` (TWAP, épsilon, perfil de liquidez, y volatility_class) کو دستاویز کرتا ہے تاکہ configuración de tiempo de ejecución de los auditores سے الگ entradas de cotización کی تصدیق کر سکیں۔

Consumidores `lane_settlement_commitments` کو موجودہ lane اور instantáneas de compromiso del espacio de datos کے ساتھ دیکھ سکتے ہیں تاکہ یہ تصدیق ہو کہ buffers de tarifas, niveles de corte de pelo, Ejecución de intercambio configurada Modelo de tarifa Nexus سے میل کھاتے ہیں۔