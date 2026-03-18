---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/signing-ceremony.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: טקס חתימה
כותרת: Sustitucion de la ceremonia de firma
תיאור: Como el Parlamento de Sora aprueba y distribuye fixtures del chunker de SoraFS (SF-1b).
sidebar_label: Ceremonia de firma
---

> מפת דרכים: **SF-1b — אפרובאציונות דה fixtures del Parlamento de Sora.**
> El flujo del Parlamento reemplaza la antigua "ceremonia de firma del consejo" במצב לא מקוון.

המדריך הטקסי של חברת ארה"ב עבור אביזרי chunker de SoraFS queda retirado. טודס
las aprobaciones ahora pasan por el **Parlamento de Sora**, la DAO basada en sorteo que gobierna
Nexus. Los miembros del Parlamento fianzan XOR para obtener ciudadania, rotan entre paneles y
emiten votos on-chain que Aprueban, rechazan o revierten releases de fixtures. Esta guia explica
el processo y el tooling עבור מפתחים.

## רזומה דל פרלמנטו

- **Ciudadania** - Los operadores fianzan el XOR requerido para inscribirse como ciudadanos y
  volverse elegibles para sorteo.
- **פאנלים** - Las responsabilidades se dividen en paneles rotativos (Infraestructura,
  Moderacion, Tesoreria, ...). El Panel de Infraestructura es el dueno de las aprobaciones de
  אביזרי דה SoraFS.
- **Sorteo y Rotacion** - Los asientos de panel se reasignan con la cadencia especificada en
  la constitucion del Parlamento para que ningun grupo monopolice las aprobaciones.

## Flujo de aprobacion de fixtures

1. **Envio de propuesta**
   - El WG de Tooling sube el bundle candidato `manifest_blake3.json` mas el diff del fixture
     al registro on-chain דרך `sorafs.fixtureProposal`.
   - La propuesta registra el digest BLAKE3, la version semantica y las notas de cambio.
2. **עדכון yvotacion**
   - El Panel de Infraestructura recibe la signacion a traves de la cola de tareas del
     פרלמנטו.
   - Los miembros del panel inspectionan artefactos de CI, corren tests de paridad y emiten
     votos ponderados על השרשרת.
3. **סיום**
   - Una vez alcanzado el quorum, el runtime emite un evento de aprobacion que incluye el
     לעכל canonico del manifest y el compromiso Merkle del last pay of fixture.
   - El evento se refleja en el registry de SoraFS para que los clientes puedan obtener el
     manifest mas reciente aprobado por el Parlamento.
4. **הפצה**
   - Los helpers de CLI (`cargo xtask sorafs-fetch-fixture`) traen el manifest aprobado desde
     Nexus RPC. Las constantes JSON/TS/Go del repositorio se mantienen sincronizadas al
     re-ejecutar `export_vectors` y validar el digest contra el registro on-chain.

## Flujo de trabajo עבור מפתחים

- אביזרי Regenera עם:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Usa el helper de fetch del Parlamento להורדת מעטפה אפרובאדו, אימות
  מקומות של מכשירי firmas y refrescar. Apunta `--signatures` al envelope publicado por el
  פרלמנטו; el helper resuelve el manifest acompanante, recomputa el digest BLAKE3 ה
  impone el perfil canonico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```Pasa `--manifest` סי el manifest vive en otra URL. Los envelopes sin firma se rechazan
salvo que se configure `--allow-unsigned` para smoke runs locales.

- Al validar un manifest a traves de un gateway de staging, apunta a Torii en vez de payloads
  מקומות:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- אל CI מקומי אין צורך בסגל `signer.json`.
  `ci/check_sorafs_fixtures.sh` השוואת אסטדו דל ריפו נגד פשרה אולטימטיבית
  על השרשרת y falla cuando divergen.

## Notas de gobernanza

- La constitucion del Parlamento gobierna quorum, rotacion y escalamiento; אין צורך
  תצורה של ארגז ניוול.
- Los rollbacks de emergencia se manejan and traves del panel de moderation del Parlamento.
  אל פאנל דה אינפראסטרוקטורה מציגה את החזרה לתשובה אל עיכול
  הקודם דל מניפסט, reemplazando la release una vez aprobada.
- Las aprobaciones historicas permanecen disponibles en el registry de SoraFS para
  שידור חוזר.

## שאלות נפוצות

- **A donde se fue `signer.json`?**  
  זה פשוט. Toda la atribucion de firmas vive on-chain; `manifest_signatures.json`
  en el repositorio es solo un fixture de developer que debe coincidir con el ultimo
  evento de aprobacion.

- **Seguimos requiriendo firmas Ed25519 locales?**  
  לא. Las aprobaciones del Parlamento se almacenan como artefactos on-chain. משחקי הפסד
  locales existen para reproducibilidad, pero se validan contra el digest del Parlamento.

- **Como monitorean las aprobaciones los equipos?**  
  הרשמה לאירוע `ParliamentFixtureApproved` או ייעוץ לרישום דרך Nexus RPC
  para recuperar el digest בפועל del Manifest y el מסדר של פאנל.