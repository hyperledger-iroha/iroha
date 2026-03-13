---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-tarifa-nexus
título: Обновления модели комиссий Nexus
descripción: Зеркало `docs/source/nexus_fee_model.md`, documentación de las personas que se encuentran en el carril y en la configuración.
---

:::nota Канонический источник
Esta página es `docs/source/nexus_fee_model.md`. Держите обе копии синхронизированными, пока мигрируют переводы на японский, иврит, испанский, португальский, FRAнцузский, русский, арабский и урду.
:::

# Обновления модели комиссий Nexus

Los enrutadores modernos funcionan con dispositivos de control remotos en el carril, los operadores pueden solucionar el problema газа с моделью комиссий Nexus.- Un enrutador de arquitecto moderno, búferes políticos, televisores de matriz y una implementación posterior. `docs/settlement-router.md`. Esta configuración, los parámetros y las opciones disponibles se incluyen en la hoja de ruta postal NX-3 y en el enrutador de monitorización SRE продакшене.
- Configuración de gas activado (`pipeline.gas.units_per_gas`) que incluye el diseño `twap_local_per_xor`, `liquidity_profile` (`tier1`, `tier2` o `tier3`) y `volatility_class` (`stable`, `elevated`, `dislocated`). Esta bandera se coloca en el enrutador de liquidación, su dispositivo XOR admite TWAP canónico y un corte de pelo rápido para el carril.
- Каждая транзакция, оплачивающая газ, записывает `LaneSettlementReceipt`. Каждый recibo хранит предоставленный вызывающим идентификатор источника, локальную микро-сумму, XOR к немедленной оплате, ожидаемый XOR после corte de pelo, фактическую variацию (`xor_variance_micro`) y метку времени блока в миллисекундах.
- El bloque de implementación agrega recibos por carril/espacio de datos y publica entre `lane_settlement_commitments` y `/v2/sumeragi/status`. Los archivos `total_local_micro`, `total_xor_due_micro` y `total_xor_after_haircut_micro` son bloques integrados para aplicaciones nocturnas.- El nuevo programa `total_xor_variance_micro` está equipado con un dispositivo de almacenamiento de datos aislado (un dispositivo que contiene XOR y ожиданием после haircut), un documento `swap_metadata` para determinar los parámetros de conversión (TWAP, épsilon, perfil de liquidez y volatility_class), чтобы аудиторы Es posible comprobar todos los parámetros que no se necesitan en las configuraciones predeterminadas.

Es posible eliminar `lane_settlement_commitments` junto con los compromisos de configuración de la línea y el espacio de datos, qué archivos adjuntos y qué buffers комиссий, уровни corte de pelo y исполнение swap соответствуют настроенной модели комиссий Nexus.