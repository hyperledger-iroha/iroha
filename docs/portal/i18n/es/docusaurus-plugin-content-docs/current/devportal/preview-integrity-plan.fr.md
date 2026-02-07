---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plan de control de visualización previa por suma de comprobación

Ce plan decrit le travail restant necessaire pour rindre cada artefacto de visualización previa del portal verificable antes de la publicación. El objetivo es garantizar que los lectores telecargan exactamente la instantánea construida en CI, que el manifiesto de suma de comprobación es inmutable y que la previsualización se puede descubrir a través de SoraFS con los metadones Norito.

## Objetivos- **Construye determinantes:** Asegúrese de que `npm run build` produzca una salida reproducible y se mantenga para siempre `build/checksums.sha256`.
- **Previsualizaciones verificadas:** Exiger que cada artefacto de previsualización proporciona un manifiesto de suma de verificación y rechaza la publicación lorsque la verificación echoue.
- **Metadonas publicadas a través de Norito:** Conserve los descriptores de visualización previa (metadonas de confirmación, resumen de suma de comprobación, CID SoraFS) en JSON Norito hasta que las herramientas de gobierno puedan auditar las versiones.
- **Operador de salida:** Introduzca un script de verificación en una etapa que los consumidores pueden ejecutar localmente (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); La encapsulación del script desorma el flujo de suma de comprobación de validación + descripción de combate y combate. El comando de visualización previa estándar (`npm run serve`) invoca el mantenimiento de este asistente automático antes de `docusaurus serve` para que las instantáneas se encuentren bajo control de suma de comprobación (con `npm run serve:verified` conserve como alias explícito).

## Fase 1: Aplicación CI1. Mettre un día `.github/workflows/docs-portal-preview.yml` para:
   - Ejecutor `node docs/portal/scripts/write-checksums.mjs` después de compilar Docusaurus (deja invocar ubicación).
   - Executer `cd build && sha256sum -c checksums.sha256` y repite el trabajo en caso de divergencia.
   - Empaquete la compilación del repertorio en `artifacts/preview-site.tar.gz`, copie el manifiesto de suma de verificación, el ejecutor `scripts/generate-preview-descriptor.mjs` y el ejecutor `scripts/sorafs-package-preview.sh` con una configuración JSON (como `docs/examples/sorafs_preview_publish.json`) para que el flujo de trabajo emette a la fois les metadonnees y un paquete SoraFS determinista.
   - Televerser le site statique, les artefactos de metadonnees (`docs-portal-preview`, `docs-portal-preview-metadata`) y le bundle SoraFS (`docs-portal-preview-sorafs`) afin que le manifeste, le resume CAR y le plan puissent etre inspectes sans relancer le build.
2. Agregue un comentario de CI de insignia que reanude el resultado de la suma de verificación de verificación en las solicitudes de extracción (implementado a través de la etapa de comentario GitHub Script de `docs-portal-preview.yml`).
3. Documente el flujo de trabajo en `docs/portal/README.md` (sección CI) y coloque las etapas de verificación en la lista de verificación de publicación.

## Script de verificación

`docs/portal/scripts/preview_verify.sh` valide los artefactos de telecarga de previsualización sin exigencia de invocaciones manuales de `sha256sum`. Utilice `npm run serve` (o el alias explícito `npm run serve:verified`) para ejecutar el script y lanzar `docusaurus serve` en una sola etapa cuando comparta instantáneas localizadas. La lógica de verificación:1. Ejecute la herramienta SHA apropiada (`sha256sum` o `shasum -a 256`) contra `build/checksums.sha256`.
2. Compare optionnellement le digest/nom de fichier du descripteur de previsualisation `checksums_manifest` et, lorsque fourni, le digest/nom de fichier de l'archive de previsualisation.
3. Ordene con un código no nulo cuando se detecte una divergencia porque los selectores pueden bloquear las previsualizaciones alteradas.

Ejemplo de utilización (después de la extracción de artefactos CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Les ingenieurs CI et release doivent appeler le script cada vez que telechargent un paquete de previsualización o adjunto des artefactos a un ticket de lanzamiento.

## Fase 2 - Publicación SoraFS

1. Etendre el flujo de trabajo de previsualización con un trabajo aquí:
   - Televerse el sitio construido frente a la pasarela de puesta en escena SoraFS utilizando `sorafs_cli car pack` y `manifest submit`.
   - Capture el resumen del manifiesto enviado y el CID SoraFS.
   - Serializar `{ commit, branch, checksum_manifest, cid }` en JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Guarde el descriptor con el artefacto de compilación y exponga el CID en el comentario de la solicitud de extracción.
3. Ajouter des tests d'integration qui exercent `sorafs_cli` en modo ensayo para asegurar que las evoluciones futuras conserven la coherencia del esquema de metadonnees.

## Fase 3: Gobernanza y auditoría1. Publique un esquema Norito (`PreviewDescriptorV1`) que decrivant la estructura del descriptor bajo `docs/portal/schemas/`.
2. Mettre a diario la lista de verificación de publicación DOCS-SORA para exigir:
   - Lancer `sorafs_cli manifest verify` en la carga CID.
   - Registre el resumen del manifiesto de suma de verificación y el CID en la descripción del PR de liberación.
3. Conecte la automatización de gobierno para croiser le descripteur con le manifeste de checksum colgante les votes de release.

## Livrables et responsabilites

| Jalón | Propietario(s) | Cíble | Notas |
|-------|-----------------|-------|-------|
| Aplicación de sumas de comprobación en libre CI | Documentos de infraestructura | Semana 1 | Agregue una puerta de validación y carga de artefactos. |
| Publicación de vistas previas SoraFS | Documentos de infraestructura / Almacenamiento de equipos | Semana 2 | Necessite l'acces aux identifiants de staging et des mises a jour du Scheme Norito. |
| Gobernanza de la integración | Documentos responsables/DevRel/Gobernanza del WG | Semana 3 | Publie le Scheme et Meet a Jour Les Checklists et Les Entrees du Roadmap. |

## Preguntas abiertas- ¿Qué entorno SoraFS doit heberger les artefactos de previsualización (puesta en escena versus carril de previsualización dediee)?
- ¿Avons-nous besoin de doubles firms (Ed25519 + ML-DSA) sur le descripteur de previsualisation antes de la publicación?
- ¿El CI de flujo de trabajo debe actualizar la configuración del orquestador (`orchestrator_tuning.json`) durante la ejecución de `sorafs_cli` para guardar los manifiestos reproducibles?

Consignez les decision dans `docs/portal/docs/reference/publishing-checklist.md` et mettez a jour ce plan une fois les inconnues resolues.