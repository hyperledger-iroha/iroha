---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-bootstrap-nexus
título: Sora Nexus بوٹ اسٹریپ اور آبزرویبیلٹی
descripción: Nexus کے بنیادی cluster validador کو آن لائن لانے سے پہلے SoraFS اور SoraNet خدمات شامل کرنے کا آپریشنل پلان۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/soranexus_bootstrap_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ورژنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus Plan de arranque y observabilidad

## اہداف
- گورننس کیز، Torii API, اور monitoreo de consenso کے ساتھ Sora Nexus validador/observador نیٹ ورک کی بنیاد قائم کریں۔
- بنیادی سروسز (Torii, consenso, persistencia) کو SoraFS/SoraNet implementaciones superpuestas سے پہلے ویلیڈیٹ کریں۔
- Flujos de trabajo de CI/CD y paneles de control/alertas de observabilidad.

## پیشگی شرائط
- Material clave de gobernanza (consejo multifirma, claves de comité) HSM یا Vault میں دستیاب ہو۔
- بنیادی انفراسٹرکچر (clústeres de Kubernetes یا nodos bare-metal) بنیادی/ثانوی ریجنز میں موجود ہو۔
- Configuración de arranque completa (`configs/nexus/bootstrap/*.toml`) y parámetros de consenso disponibles## نیٹ ورک ماحول
- Entornos Nexus کو الگ نیٹ ورک prefijos کے ساتھ چلائیں:
- **Sora Nexus (mainnet)** - پروڈکشن نیٹ ورک prefijo `nexus` جو gobernanza canónica اور SoraFS/SoraNet servicios piggyback میزبان بناتا ہے (cadena ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - puesta en escena نیٹ ورک prefijo `testus` جو configuración de la red principal کو pruebas de integración اور validación previa al lanzamiento کے لئے mirror کرتا ہے (UUID de cadena `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- ہر entorno کے لئے الگ archivos de génesis, claves de gobernanza, اور huellas de infraestructura رکھیں۔ Testus تمام SoraFS/SoraNet implementaciones کے لئے campo de pruebas ہے، Nexus میں promover کرنے سے پہلے۔
- Canalizaciones de CI/CD پہلے Testus پر implementar کریں، pruebas de humo automatizadas چلائیں، اور comprobaciones پاس ہونے پر Nexus میں promoción manual درکار ہو۔
- Paquetes de configuración de referencia `configs/soranexus/nexus/` (mainnet) y `configs/soranexus/testus/` (testnet) Torii directorios de admisión شامل ہیں۔## مرحلہ 1 - Revisión de configuración
1. موجودہ documentación کا auditoría کریں:
   - `docs/source/nexus/architecture.md` (consenso, diseño Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestructura).
   - `docs/source/nexus/governance_keys.md` (procedimientos de custodia de claves).
2. Archivos Génesis (`configs/nexus/genesis/*.json`) کو validar کریں کہ وہ موجودہ lista de validadores اور apostar pesos سے alinear ہیں۔
3. نیٹ ورک parámetros کنفرم کریں:
   - Tamaño del comité de consenso y quórum.
   - Umbrales de intervalo/finalidad de bloque.
   - Puertos de servicio Torii y certificados TLS.

## Paso 2: Implementación del clúster Bootstrap
1. Provisión de nodos validadores کریں:
   - Instancias `irohad` (validadores) کو volúmenes persistentes کے ساتھ implementar کریں۔
   - نیٹ ورک reglas de firewall کو یقینی بنائیں کہ consenso اور Torii nodos de tráfico کے درمیان permitidos ہو۔
2. ہر validador پر Torii servicios (REST/WebSocket) کو TLS کے ساتھ شروع کریں۔
3. اضافی resiliencia کے لئے nodos observadores (solo lectura) implementan کریں۔
4. Scripts de arranque (`scripts/nexus_bootstrap.sh`) چلائیں تاکہ génesis تقسیم ہو، consenso شروع ہو، اور nodos رجسٹر ہوں۔
5. Pruebas de humo چلائیں:
   - Torii کے ذریعے las transacciones de prueba envían کریں (`iroha_cli tx submit`).
   - Telemetría کے ذریعے producción de bloques/finalidad verificar کریں۔
   - Validadores/observadores کے درمیان replicación del libro mayor چیک کریں۔## مرحلہ 3 - Gobernanza y gestión de claves
1. Configuración del consejo multifirma لوڈ کریں؛ تصدیق کریں کہ propuestas de gobernanza presentar اور ratificar ہو سکتی ہیں۔
2. Claves del consenso/comité کو محفوظ رکھیں؛ acceder al registro کے ساتھ copias de seguridad automáticas configurar کریں۔
3. Procedimientos de rotación de claves de emergencia (`docs/source/nexus/key_rotation.md`) سیٹ اپ کریں اور runbook verificar کریں۔

## مرحلہ 4 - Integración CI/CD
1. Las tuberías configuran کریں:
   - Imágenes de Validator/Torii compiladas y publicadas (GitHub Actions y GitLab CI).
   - Validación de configuración automatizada (génesis de pelusa, verificación de firmas).
   - Canales de implementación (Helm/Kustomize), puesta en escena y clusters de producción.
2. Pruebas de humo de CI میں شامل کریں (grupo efímero اٹھائیں، conjunto de transacciones canónicas چلائیں).
3. Implementaciones de ناکام کے لئے scripts de reversión شامل کریں اور runbooks دستاویز کریں۔## مرحلہ 5 - Observabilidad y alertas
1. Pila de monitoreo (Prometheus + Grafana + Alertmanager) ہر región میں implementar کریں۔
2. Métricas básicas:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Torii اور servicios de consenso کے لئے Loki/ELK کے ذریعے logs۔
3. Paneles de control:
   - Estado del consenso (altura del bloque, finalidad, estado de los pares).
   - Tasas de error/latencia de API Torii.
   - Transacciones de gobernanza y estados de propuestas.
4. Alertas:
   - Parada de producción de bloques (>2 intervalos de bloques).
   - Quórum de recuento de pares سے نیچے گر جائے۔
   - Picos de tasa de error Torii.
   - Acumulación de cola de propuestas de gobernanza.

## مرحلہ 6 - Validación y transferencia
1. Validación de un extremo a otro:
   - Envío de propuesta de gobernanza کریں (cambio de parámetro مثلاً).
   - Aprobación del Consejo کے ذریعے proceso کریں تاکہ proceso de gobernanza درست ہو۔
   - Diferencia de estado del libro mayor چلائیں تاکہ consistencia یقینی ہو۔
2. Runbook de guardia دستاویز کریں (respuesta a incidentes, conmutación por error, escalamiento).
3. SoraFS/SoraNet ٹیموں کو preparación بتائیں؛ Implementaciones adicionales de nodos Nexus y puntos de implementación## Lista de verificación de implementación
- [] Génesis/auditoría de configuración مکمل۔
- [] Validador y nodos observadores implementan y consenso.
- [] Claves de gobernanza لوڈ ہوئے، prueba de propuesta ہوا۔
- [] Canalizaciones de CI/CD چل رہے ہیں (construcción + implementación + pruebas de humo).
- [] Paneles de observabilidad فعال ہیں اور alertas موجود ہے۔
- [] Documentación de transferencia en sentido descendente ٹیموں کو دے دی گئی۔