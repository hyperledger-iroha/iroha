---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "SoraFS مائیگریشن روڈ میپ"
---

> [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md) سے ماخوذ۔

# SoraFS مائیگریشن روڈ میپ (SF-1)

یہ دستاویز `docs/source/sorafs_architecture_rfc.md` میں بیان کردہ مائیگریشن رہنمائی کو
عملی بناتی ہے۔ یہ SF-1 کے entregables کو ایسے hitos, criterios de puerta اور propietario
listas de verificación میں توسیع دیتی ہے جو اجرا کے لیے تیار ہوں، تاکہ almacenamiento, gobernanza, DevRel
ہم آہنگ کر سکیں۔

یہ روڈ میپ جان بوجھ کر determinista ہے: ہر hito مطلوبہ artefactos، comando
invocaciones اور pasos de certificación کو نام دیتا ہے تاکہ tuberías posteriores ایک جیسے
salidas بنائیں اور gobernanza کے پاس rastro auditable برقرار رہے۔

## Descripción general de los hitos

| Hito | Ventana | Objetivos principales | Debe enviar | Propietarios |
|-----------|--------|---------------|-----------|--------|
| **M1 - Aplicación determinista** | Semanas 7-12 | accesorios firmados نافذ کرنا اور alias pruebas etapa کرنا جب tuberías expectativa banderas اپنائیں۔ | verificación de accesorios nocturnos, manifiestos firmados por el consejo, alias entradas de preparación del registro. | Almacenamiento, Gobernanza, SDK |

Estado del hito `docs/source/sorafs/migration_ledger.md` میں ٹریک ہوتا ہے۔ اس روڈ میپ میں
ہر تبدیلی لازمی طور پر libro mayor کو اپڈیٹ کرے تاکہ gobernanza اور ingeniería de lanzamiento ہم آہنگ رہیں۔

## Flujos de trabajo

### 2. Adopción de fijación determinista| Paso | Hito | Descripción | Propietario(s) | Salida |
|------|-----------|-------------|----------|--------|
| Ensayos de accesorios | M0 | ہفتہ وار simulacros جو resúmenes de fragmentos locales کو `fixtures/sorafs_chunker` کے ساتھ comparar کریں۔ `docs/source/sorafs/reports/` میں رپورٹ شائع کریں۔ | Proveedores de almacenamiento | `determinism-<date>.md` matriz pasa/falla کے ساتھ۔ |
| Hacer cumplir las firmas | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` اس وقت fallan ہوں جب firmas یا manifiestos deriva کریں۔ El desarrollador anula la exención de gobernanza PR کے ساتھ منسلک ہونا چاہیے۔ | Grupo de Trabajo sobre Herramientas | Registro de CI, enlace del ticket de exención (اگر لاگو ہو). |
| Banderas de expectativa | M1 | Tuberías `sorafs_manifest_stub` کو expectativas explícitas کے ساتھ کال کرتے ہیں تاکہ pin de salida ہوں: | Documentos CI | Scripts actualizados que hacen referencia a indicadores de expectativa (نیچے bloque de comando دیکھیں). |
| Fijación de registro primero | M2 | Envíos de manifiesto `sorafs pin propose` y `sorafs pin approve` کو wrap کرتے ہیں؛ CLI predeterminado `--require-registry` ہے۔ | Operaciones de gobernanza | Registro de auditoría de CLI de registro, propuestas fallidas y telemetría. |
| Paridad de observabilidad | M3 | Alerta de paneles Prometheus/Grafana کرتے ہیں جب manifiestos de registro de inventarios de fragmentos سے diverge ہوں؛ alertas operaciones de guardia سے جڑے ہوں۔ | Observabilidad | Enlace al panel, ID de reglas de alerta, resultados de GameDay. |

#### Comando de publicación canónica

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

resumen, tamaño اور CID کو انہی متوقع حوالوں سے بدلیں جو entrada del libro mayor de migración میں ریکارڈ ہیں۔### 3. Transición de alias a comunicaciones

| Paso | Hito | Descripción | Propietario(s) | Salida |
|------|-----------|-------------|----------|--------|
| Pruebas de alias en la puesta en escena | M1 | Pin Registro de puesta en escena میں alias afirma رجسٹر کریں اور manifiestos کے ساتھ Pruebas de Merkle لگائیں (`--alias`). | Gobernanza, Documentos | Manifiesto del paquete de prueba کے ساتھ محفوظ + comentario del libro mayor میں alias نام۔ |
| Ejecución de pruebas | M2 | Gateways ایسے manifiestos کو rechazar کریں جن میں تازہ `Sora-Proof` encabezados نہ ہوں؛ CI میں `sorafs alias verify` paso شامل کریں۔ | Redes | Parche de configuración de puerta de enlace + salida CI جس میں verificación exitosa ہو۔ |

### 4. Comunicación y auditoría

- **Disciplina del libro mayor:** ہر cambio de estado (desviación de accesorios, envío de registro, activación de alias)
  کو `docs/source/sorafs/migration_ledger.md` میں nota fechada کے طور پر anexar کرنا لازمی ہے۔
- **Actas de gobernanza:** sesiones del consejo, cambios en el registro de pines y políticas de alias aprobadas کریں
  انہیں اس hoja de ruta اور libro mayor کا حوالہ دینا چاہیے۔
- **Comunicaciones externas:** DevRel ہر hito پر actualizaciones de estado شائع کرتا ہے (blog + extracto del registro de cambios)
  جو garantías deterministas اور alias líneas de tiempo کو نمایاں کریں۔

## Dependencias y riesgos| Dependencia | Impacto | Mitigación |
|------------|--------|------------|
| Disponibilidad del contrato de Registro de PIN | Despliegue del primer pin M2 کو بلاک کرتی ہے۔ | M2 سے پہلے pruebas de repetición کے ساتھ etapa del contrato کریں؛ regresiones ختم ہونے تک sobre de reserva برقرار رکھیں۔ |
| Claves de firma del Consejo | Sobres de manifiesto اور aprobaciones de registro کے لیے ضروری۔ | Ceremonia de firma `docs/source/sorafs/signing_ceremony.md` میں دستاویزی ہے؛ superponer کے ساتھ teclas rotar کریں اور nota del libro mayor رکھیں۔ |
| Cadencia de lanzamiento del SDK | Clientes کو M3 سے پہلے pruebas de alias پر عمل کرنا ہوگا۔ | Ventanas de lanzamiento del SDK کو puertas de hitos کے ساتھ align کریں؛ listas de verificación de migración کو plantillas de lanzamiento میں شامل کریں۔ |

Riesgos residuales y mitigaciones `docs/source/sorafs_architecture_rfc.md` میں بھی درج ہیں
اور ajustes کے وقت referencia cruzada کرنا چاہیے۔

## Lista de verificación de criterios de salida

| Hito | Criterios |
|-----------|----------|
| M1 | - Trabajo fijo nocturno سات دن مسلسل green۔  - Pruebas de alias de preparación CI میں verificar۔  - Política de indicadores de expectativas de gobernanza کی توثیق کرے۔ |

## Gestión del cambio

1. PR کے ذریعے تبدیلیاں تجویز کریں اور یہ فائل **اور**
   `docs/source/sorafs/migration_ledger.md` دونوں اپڈیٹ کریں۔
2. Descripción de relaciones públicas میں actas de gobernanza اور evidencia de CI کے enlaces شامل کریں۔
3. fusionar almacenamiento + lista de correo DevRel resumen y acciones esperadas del operadorیہ طریقہ کار یقینی بناتا ہے کہ SoraFS implementación determinista, auditable, اور transparente رہے
اور Nexus لانچ میں شریک ٹیموں کے درمیان ہم آہنگی برقرار رہے۔