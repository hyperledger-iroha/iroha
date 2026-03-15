---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elástico-carril
título: لچکدار carril پروویژنگ (NX-7)
sidebar_label: carril لچکدار پروویژنگ
descripción: Manifiestos de carril Nexus, entradas de catálogo, evidencia de implementación بنانے کے لئے bootstrap ورک فلو۔
---

:::nota Fuente canónica
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ جب تک ترجمہ پورٹل تک نہیں پہنچتا، دونوں کاپیوں کو alineado رکھیں۔
:::

# لچکدار carril پروویژنگ ٹول کٹ (NX-7)

> **Elemento de la hoja de ruta:** NX-7: herramientas de carril de لچکدار پروویژنگ  
> **Estado:** herramientas مکمل - manifiestos, fragmentos de catálogo, cargas útiles Norito, pruebas de humo بناتا ہے،
> Asistente de paquete de pruebas de carga, activación de latencia de ranuras + manifiestos de evidencia, validadores y ejecuciones de carga
> بغیر مخصوص scripting کے شائع کیے جا سکیں۔

یہ گائیڈ operadores کو نئے `scripts/nexus_lane_bootstrap.sh` helper کے ذریعے لے جاتی ہے جو generación de manifiesto de carril, fragmentos de catálogo de carril/espacio de datos, اور evidencia de implementación کو خودکار بناتا ہے۔ مقصد یہ ہے کہ نئی Nexus carriles (públicos یا privados) متعدد فائلیں دستی طور پر editar کیے بغیر اور geometría del catálogo دوبارہ ہاتھ سے deriva کیے بغیر آسانی سے بنائی جا سکیں۔

## 1. Requisitos previos1. alias de carril, espacio de datos, conjunto de validadores, tolerancia a fallas (`f`), política de liquidación y aprobación de gobernanza
2. validadores کی حتمی فہرست (ID de cuenta) اور espacios de nombres protegidos کی فہرست۔
3. repositorio de configuración de nodos تک رسائی تاکہ fragmentos generados شامل کیے جا سکیں۔
4. registro de manifiesto de carril کے لئے rutas (دیکھیں `nexus.registry.manifest_directory` اور `cache_directory`).
5. carril کے لئے contactos de telemetría/PagerDuty maneja تاکہ alertas carril کے en línea ہوتے ہی cableado ہو سکیں۔

## 2. artefactos de carril بنائیں

raíz del repositorio سے ayudante چلائیں:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Banderas clave:

- `--lane-id` کو `nexus.lane_catalog` میں نئے entrada کے índice سے coincidencia ہونا چاہیے۔
- `--dataspace-alias` اور `--dataspace-id/hash` entrada de catálogo de espacio de datos کو control کرتے ہیں (omitir ہونے پر ID de carril predeterminado استعمال ہوتا ہے).
- `--validator` کو repetir کیا جا سکتا ہے یا `--validators-file` سے پڑھا جا سکتا ہے۔
- Las reglas de enrutamiento listas para pegar `--route-instruction` / `--route-account` emiten کرتے ہیں۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) captura de contactos del runbook کرتے ہیں تاکہ paneles de control درست propietarios دکھائیں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` gancho de actualización de tiempo de ejecución کو manifiesto میں شامل کرتے ہیں جب lane کو controles de operador extendidos درکار ہوں۔
- `--encode-space-directory` خودکار طور پر `cargo xtask space-directory encode` چلاتا ہے۔ `--space-directory-out` کے ساتھ استعمال کریں جب `.to` فائل کو default کے علاوہ کسی اور جگہ رکھنا ہو۔اس اسکرپٹ سے `--output-dir` کے اندر تین artefactos بنتے ہیں (directorio de directorio predeterminado ہے), اور codificación فعال ہونے پر ایک چوتھا بھی بنتا ہے:

1. `<slug>.manifest.json`: manifiesto de carril, quórum de validación, espacios de nombres protegidos, metadatos del gancho de actualización de tiempo de ejecución, etc.
2. `<slug>.catalog.toml` - Fragmento TOML de `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`, reglas de enrutamiento y reglas de enrutamiento entrada de espacio de datos میں `fault_tolerance` لازمی طور پر set کریں تاکہ comité de retransmisión de carril (`3f+1`) درست سائز ہو۔
3. `<slug>.summary.json` - resumen de auditoría, geometría (slug, segmentos, metadatos) y pasos de implementación requeridos y `cargo xtask space-directory encode` y comando exacto (`space_directory_encode.command`, pasos) ہے۔ اسے billete de embarque کے ساتھ evidencia کے طور پر adjuntar کریں۔
4. `<slug>.manifest.to` - `--encode-space-directory` فعال ہونے پر بنتا ہے؛ Torii کے `iroha app space-directory manifest publish` flujo کے لئے listo ہے۔

`--dry-run`, JSON/snippets, vista previa, `--force`, sobrescritura de artefactos

## 3. تبدیلیاں لاگو کریں1. manifiesto JSON configurado `nexus.registry.manifest_directory` میں کاپی کریں (اور directorio de caché میں بھی اگر registro de paquetes remotos espejo کرتا ہے). اگر manifiesta el repositorio de configuración میں versionado ہوں تو فائل commit کریں۔
2. fragmento de catálogo کو `config/config.toml` (یا مناسب `config.d/*.toml`) میں anexar کریں۔ `nexus.lane_count` کم از کم `lane_id + 1` ہونا چاہیے اور نئے carril کے لئے `nexus.routing_policy.rules` اپ ڈیٹ کریں۔
3. Codifique کریں (اگر `--encode-space-directory` چھوڑا تھا) اور Space Directory میں manifiesto publicar کریں۔ resumen میں موجود comando (`space_directory_encode.command`) استعمال کریں۔ اس سے `.manifest.to` carga útil بنتا ہے اور auditors کے لئے evidencia ریکارڈ ہوتا ہے؛ `iroha app space-directory manifest publish` کے ذریعے enviar کریں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور salida de seguimiento کو ticket de implementación میں archivo کریں۔ یہ ثابت کرتا ہے کہ نئی geometría generada slug/segmentos de Kura کے مطابق ہے۔
5. manifiesto/catálogo تبدیلیاں implementar ہونے کے بعد carril کے لئے validadores asignados کو reiniciar کریں۔ auditorías futuras کے لئے resumen JSON کو ticket میں رکھیں۔

## 4. paquete de distribución de registro بنائیں

manifiesto generado اور superposición کو paquete کریں تاکہ operadores ہر host پر configuraciones editar کیے بغیر datos de gobierno de carril distribuir کر سکیں۔ manifiestos de ayuda del paquete ہے:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Salidas:1. `manifests/<slug>.manifest.json` - انہیں configurado `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` میں رکھیں۔ ہر Entrada `--module` ایک definición de módulo conectable بن جاتی ہے، جس سے intercambios de módulo de gobierno (NX-2) ممکن ہوتے ہیں اور صرف superposición de caché اپ ڈیٹ کرنا پڑتا ہے، `config.toml` میں ترمیم نہیں۔
3. `summary.json`: hashes, metadatos superpuestos, instrucciones del operador شامل ہیں۔
4. Opcional `registry_bundle.tar.*` - SCP، S3، یا rastreadores de artefactos کے لئے listo۔

Directorio principal (archivo) Validador Sincronización Hosts con espacio de aire Extracto Torii Reinicio Manifestaciones + superposición de caché Rutas de registro میں کاپی کریں۔

## 5. Pruebas de humo del validador

Torii reiniciar کے بعد نیا ayudante de humo چلائیں تاکہ carril `manifest_ready=true` رپورٹ کرے، métricas میں recuento de carriles esperado نظر آئے، اور medidor sellado صاف ہو۔ جن lanes کو manifiesta درکار ہوں انہیں no vacío `manifest_path` ظاہر کرنا چاہیے؛ ayudante اب ruta غائب ہونے پر فوراً falla ہوتا ہے تاکہ ہر implementación de NX-7 میں evidencia manifiesta firmada شامل ہو:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```Entornos autofirmados میں ٹیسٹ کرتے وقت `--insecure` شامل کریں۔ اگر carril غائب ہو، sellado ہو، یا valores esperados de métricas/telemetría سے deriva کریں تو script de salida distinta de cero کرتا ہے۔ `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, y `--max-headroom-events` استعمال کریں تاکہ telemetría de altura de bloque a nivel de carril/finalidad/acumulación/espacio libre آپ کے envolvente operativa میں رہے، اور `--max-slot-p95` / `--max-slot-p99` (ساتھ `--min-slot-samples`) کے ساتھ ملا کر Ayudante de objetivos de duración de ranura NX-18 کے اندر ہی aplicar کریں۔

Validaciones con espacio de aire (یا CI) کے لئے آپ punto final en vivo پر جانے کے بجائے capturada reproducción de respuesta Torii کر سکتے ہیں:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` کے dispositivos grabados bootstrap helper کے تیار کردہ artefactos کی عکاسی کرتے ہیں تاکہ نئے manifests کو بغیر مخصوص scripting کے pelusa کیا جا سکے۔ CI اسی flow کو `ci/check_nexus_lane_smoke.sh` اور `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) کے ذریعے چلاتا ہے تاکہ ثابت ہو کہ Ayudante de humo NX-7 شائع شدہ formato de carga útil کے مطابق رہتا ہے اور resúmenes/superposiciones de paquetes reproducibles رہتے ہیں۔