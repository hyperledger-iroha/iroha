---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plano de previsualizacao con suma de comprobación

Este plano descreve el trabajo restante necesario para tornar cada artefato de previsualización del portal verificavel antes de la publicación. El objetivo y garantizar que los revisores baixem exactamente o snapshot construidos en CI, que o manifesto de checksum seja imutavel y que a pre-visualizacao seja descobrivel via SoraFS con metadados Norito.

## Objetivos

- **Construye determinísticas:** Garantir que `npm run build` produza Saida reproduzivel e sempre gere `build/checksums.sha256`.
- **Pre-visualizacoes verificadas:** Exigir que cada artefato de pre-visualizacao incluya un manifiesto de checksum e recusar a publicacao quando a verificacao falhar.
- **Metadados publicados vía Norito:** Persistir descritos de pre-visualizacao (metados de commit, digest de checksum, CID SoraFS) como JSON Norito para que como herramientas de gobierno possam auditar releases.
- **Funciones para operadores:** Fornecer um script de verificacao de um passo que los consumidores possam ejecutan localmente (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); El script agora involucra el flujo de validación de checksum +descriptor de ponta a ponta. El comando padrao de pre-visualizacao (`npm run serve`) ahora invoca esse helper automáticamente antes de `docusaurus serve` para que las instantáneas localicen permanentemente protegidas por checksum (con `npm run serve:verified` mantido como alias explícito).## Fase 1 - Aplicación en CI

1. Actualizar `.github/workflows/docs-portal-preview.yml` para:
   - Ejecutar `node docs/portal/scripts/write-checksums.mjs` después de compilar Docusaurus (ya invocado localmente).
   - Ejecutar `cd build && sha256sum -c checksums.sha256` y falhar o job en caso de divergencia.
   - Empacar el directorio build como `artifacts/preview-site.tar.gz`, copiar el manifiesto de suma de comprobación, ejecutar `scripts/generate-preview-descriptor.mjs` y ejecutar `scripts/sorafs-package-preview.sh` con una configuración JSON (ver `docs/examples/sorafs_preview_publish.json`) para que el flujo de trabajo emita tantos metadados como un paquete. SoraFS determinístico.
   - Enviar al sitio estático, los artefatos de metadados (`docs-portal-preview`, `docs-portal-preview-metadata`) y el paquete SoraFS (`docs-portal-preview-sorafs`) para que el manifiesto, el currículum CAR y el plano possam ser inspecionados sin refazer o build.
2. Agregar un comentario de insignia CI resumiendo el resultado de la verificación de suma de comprobación de las solicitudes de extracción (implementado mediante el paso de comentario GitHub Script de `docs-portal-preview.yml`).
3. Documentar el flujo de trabajo en `docs/portal/README.md` (secao CI) y vincular las etapas de verificación en la lista de verificación de publicación.

## Script de verificación

`docs/portal/scripts/preview_verify.sh` valida los artefactos de previsualización bajos sin requerir invocacoes manuais de `sha256sum`. Utilice `npm run serve` (o el alias explícito `npm run serve:verified`) para ejecutar el script e iniciar `docusaurus serve` con un único paso para compartir instantáneas locales. Una lógica de verificación:1. Ejecute una herramienta SHA apropiada (`sha256sum` o `shasum -a 256`) contra `build/checksums.sha256`.
2. Opcionalmente compare o digest/nome de arquivo do descriptor de pre-visualizacao `checksums_manifest` e, quando fornecido, o digest/nome de arquivo do arquivo de pre-visualizacao.
3. Sai com codigo nao zero quando alguma divergencia e detectada para que revisores possam bloquear pre-visualizacoes adulteradas.

Ejemplo de uso (apos extrair os artefatos de CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Los ingenieros de CI y lanzamiento deben ejecutar el script siempre que baixarem um paquete de previsualización o anexarem artefatos a un ticket de lanzamiento.

## Fase 2 - Publicación en SoraFS

1. Estender o flujo de trabajo de previsualización con un trabajo que:
   - Envie o site construido para o gateway de staging do SoraFS usando `sorafs_cli car pack` e `manifest submit`.
   - Captura del resumen del manifiesto retornado y del CID del SoraFS.
   - Serializa `{ commit, branch, checksum_manifest, cid }` en JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Armazenar o describir junto al artefato de build e export o CID sin comentario do pull request.
3. Agregue pruebas de integración que exercitem `sorafs_cli` en modo dry-run para garantizar que mudancas futuras mantengan la consistencia del esquema de metadados.

## Fase 3 - Gobernanza y auditorios1. Publicar un esquema Norito (`PreviewDescriptorV1`) descrevendo a estructura do descritor em `docs/portal/schemas/`.
2. Actualizar una lista de verificación de publicación DOCS-SORA para exigir:
   - Rodar `sorafs_cli manifest verify` contra o CID enviado.
   - Registrar o digest do manifesto de checksum e o CID na descricao do PR de release.
3. Ligar a automacao degobernanza para cruzar o descritor com o manifesto de checksum durante votos de liberación.

## Entregaveis e responsabilidade

| marco | Propietario(s) | alvo | Notas |
|-------|-----------------|------|-------|
| Aplicación de suma de comprobación en CI concluida | Infraestructura de Documentos | Semana 1 | Adiciona gate de falha e uploads de artefatos. |
| Publicación de previsualización no SoraFS | Infraestrutura de Documentos / Equipo de Almacenamiento | Semana 2 | Solicite acceso a las credenciales de puesta en escena y actualice el esquema Norito. |
| Integracao de Gobernanza | Líder de Docs/DevRel / WG de Gobernanza | Semana 3 | Publica el esquema y actualiza las listas de verificación y las entradas de la hoja de ruta. |

## Perguntas en el espacio abierto- ¿Qué ambiente do SoraFS deve hospedar artefatos de pre-visualizacao (puesta en escena vs. carril de pre-visualizacao dedicado)?
- Precisamos de assinaturas duplas (Ed25519 + ML-DSA) no descritor de pre-visualizacao antes da publicacao?
- ¿El flujo de trabajo de CI debe arreglar la configuración del orquestador (`orchestrator_tuning.json`) y ejecutar `sorafs_cli` para reproducir manifiestos?

Registre as decisoes em `docs/portal/docs/reference/publishing-checklist.md` e actualice este plano cuando duvidas forem resolvidas.