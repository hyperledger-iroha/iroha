---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: carta-registro-fragmento
título: Carta de registro de fragmentador de SoraFS
sidebar_label: Carta de registro de fragmentador
descripción: Carta de gobierno para submissao e aprovacao de perfis de chunker.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/chunker_registry_charter.md`. Mantenha ambas como copias sincronizadas.
:::

# Carta de gobierno del registro de fragmentador de SoraFS

> **Ratificado:** 2025-10-29 pelo Panel de Infraestructura del Parlamento de Sora (veja
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Qualquer emenda requer um
> voto formal de gobernancia; equipes de implementacao devem tratar este documento como
> normativo ate que uma carta sustituta seja aprobada.

Esta carta define el proceso y los documentos para evolucionar el registro de fragmentador de SoraFS.
Ela complementa o [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md) ao descrever como novos
perfis sao propostos, revisados, ratificados y eventualmente descontinuados.

##escopo

La carta se aplica a cada entrada en `sorafs_manifest::chunker_registry` e
una herramienta de calidad que consome o registro (CLI de manifiesto, CLI de anuncio de proveedor,
SDK). Ela impoe os invariantes de alias e handle verificados por
`chunker_registry::ensure_charter_compliance()`:- ID de perfil sao inteiros positivos que aumentan de forma monotona.
- O handle canonico `namespace.name@semver` **deve** aparecer como a primeira
  entrada en `profile_aliases`. Alias ​​alternativos vem em seguida.
- Como cadenas de alias sao aparadas, unicas e nao colidem com maneja canónicos
  de otras entradas.

##Papeis

- **Autor(es)** - preparar una propuesta, regenerar accesorios y coletam a
  evidencia de determinismo.
- **Grupo de Trabajo sobre Herramientas (TWG)** - valida una propuesta usando as checklists
  publicadas e assegura que os invariantes do registro sejam atendidos.
- **Consejo de Gobernanza (GC)** - revisa el relatorio del GTT, assina o sobre da propuesta
  e aprova os prazos de publicacao/deprecacao.
- **Equipo de almacenamiento** - mantem a implementacao do registro e publica
  actualizacoes de documentacao.

## Flujo del ciclo de vida

1. **Presentación de propuesta**
   - El autor ejecuta la lista de verificación de validación de la guía de autoria y cria.
     um JSON `ChunkerProfileProposalV1` sollozo
     `docs/source/sorafs/proposals/`.
   - Incluye la siguiente CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envie um PR contendo fixtures, proposta, relatorio de determinismo e
     actualizacoes do registro.2. **Revisión de herramientas (TWG)**
   - Repita una lista de verificación de validación (accesorios, fuzz, canalización de manifiesto/PoR).
   - Ejecute `cargo test -p sorafs_car --chunker-registry` y garanta que
     `ensure_charter_compliance()` pase con una nueva entrada.
   - Verifique que el comportamiento de CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflita os alias e identificadores atualizados.
   - Produza um relatorio curto resumindo achados e status de aprovacao/reprovacao.

3. **Aprovacao do conselho (GC)**
   - Revisar el informe del TWG y los metadados de la propuesta.
   - Assine o digest da proposta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     e anexo como assinaturas ao sobre do conselho mantido junto aos accesorios.
   - Registre o resultado da votacao nas atas degobernanca.

4. **Publicación**
   - Faca merge do PR, actualizando:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentacao (`chunker_registry.md`, guias de autoria/conformidade).
     - Calendarios y relatorios de determinismo.
   - Notifique a los operadores y equipe el SDK sobre el nuevo perfil y el planizado de implementación.

5. **Deprecacao / Encerramento**
   - Propostas que sustituem um perfil existente devem incluir uma janela de publicacao
     dupla (periodos de carencia) e um plano de actualización.
     no registro e actualize o ledger de migracao.6. **Mudancas de emergencia**
   - Las eliminaciones o revisiones exigen el voto del consejo con la aprobación de la mayor parte.
   - El TWG debe documentar las etapas de mitigación de riesgos y actualizar el registro de incidentes.

## Expectativas de herramientas

- Expoema `sorafs_manifest_chunk_store` e `sorafs_manifest_stub`:
  - `--list-profiles` para inspeccionar el registro.
  - `--promote-profile=<handle>` para gerar o bloque de metadados canónico usado
    También promover un perfil.
  - `--json-out=-` para transmitir relatos para stdout, habilitando registros de revisión
    reproducimos.
- `ensure_charter_compliance()` e invocado na inicializacao dos binarios relevantes
  (`manifest_chunk_store`, `provider_advert_stub`). Los testículos de CI devem falhar se
  nuevas entradas violarem a carta.

##Registro

- Armazene todos los relatos de determinismo em `docs/source/sorafs/reports/`.
- As atas do conselho que referenciam decisoes de chunker ficam em
  `docs/source/sorafs/migration_ledger.md`.
- Atualize `roadmap.md` e `status.md` apos cada mudanca mayor no registro.

## Referencias

- Guía de autoria: [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md)
- Lista de verificación de conformidad: `docs/source/sorafs/chunker_conformance.md`
- Referencia del registro: [Registro de perfis de chunker](./chunker-registry.md)