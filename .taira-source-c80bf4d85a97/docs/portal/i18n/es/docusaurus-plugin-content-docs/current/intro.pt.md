---
lang: es
direction: ltr
source: docs/portal/docs/intro.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bem vindo ao Portal de Desenvolvedores SORA Nexus

El portal de desarrolladores SORA Nexus reúne documentación interactiva, tutoriales de SDK y referencias de API para operadores Nexus y contribuyentes Hyperledger Iroha. Ele complementa el sitio principal de documentos para exportar guías prácticas y especificaciones geradas directamente desde este repositorio. Una página de destino ahora cuenta con puntos de entrada temáticos de Norito/SoraFS, instantáneas OpenAPI assinados y una referencia dedicada a Norito Streaming para que contribuyentes encontraron o contrato do control-plane de streaming sem vasculhar a spec raiz.

## O que voce pode fazer aquí

- **Aprender Norito** - comience con la descripción general y el inicio rápido para comprender el modelo de serialización y las herramientas de código de bytes.
- **Inicializar SDK** - siga los inicios rápidos de JavaScript y Rust hoy; Las guías de Python, Swift y Android serán adicionados conforme a las recetas de sus migradas.
- **Explorar referencias de API** - La página OpenAPI de Torii renderiza específicamente REST más recientemente y contiene tablas de configuración para fuentes canónicas en Markdown.
- **Preparar despliegues** - runbooks operativos (telemetría, liquidación, superposiciones Nexus) están siendo portados de `docs/source/` y chegarao a este sitio conforme a migracao avancar.

## Estado actual- OK Landing Docusaurus v3 tematizada con tipografía renovada, hero/cards guiadas por gradiente y mosaicos de recursos que incluyen o resumen de Norito Streaming.
- OK Plugin OpenAPI do Torii ligado a `npm run sync-openapi`, con comprobaciones de instantáneas asociadas y guardias CSP aplicadas por `buildSecurityHeaders`.
- OK Vista previa y cobertura de la sonda rodam no CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), ahora gateando o doc de streaming, os inicios rápidos de SoraFS y como listas de verificación de referencia antes de la publicación de artefatos.
- OK Quickstarts de Norito, SoraFS y SDK más secos de referencia están en la barra lateral; novas importacoes de `docs/source/` (streaming, Orchestration, runbooks) chegam aqui conforme sao escritas.

## Como participar

- Veja `docs/portal/README.md` para comandos de desarrollo local (`npm install`, `npm run start`, `npm run build`).
- As tarefas de migracao de conteudo sao acompanhadas junto aos itens de roadmap `DOCS-*`. Contribuícoes sao bem-vindas - porte secoes de `docs/source/` y adicione a la página na sidebar.
- Se voce adicionar um artefato gerado (especificaciones, tablas de configuración), documentar o comando de compilación para que futuros contribuyentes puedan actualizarlo fácilmente.