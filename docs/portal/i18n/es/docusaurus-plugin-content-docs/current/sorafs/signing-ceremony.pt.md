---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ceremonia de firma
título: Substituicao da cerimonia de assinatura
descripción: Como o Parlamento Sora aprova e distribui accesorios do fragmentador SoraFS (SF-1b).
sidebar_label: Cerimonia de assinatura
---

> Hoja de ruta: **SF-1b - aprobación de los partidos del Parlamento Sora.**
> O fluxo do Parlamento sustituyei a antiga "cerimonia de assinatura do conselho" fuera de línea.

O ritual manual de assinatura usado para los dispositivos do chunker SoraFS foi aposentado.
Todas las aprovacoes agora passam pelo **Parlamento Sora**, a DAO basado en sorteio que
gobierno o Nexus. Miembros del Parlamento bloquean XOR para obtener ciudadania, rotacionam
entre doloreis y votam on-chain para aprobar, rejeitar o reverter lanzamientos de accesorios.
Esta guía explica el proceso y las herramientas para desarrolladores.

## Visao general del Parlamento

- **Cidadania** - Operadores bloquean o XOR necesarios para inscrever como cidados e
  se tornar elegiveis ao sorteio.
- **Paineis** - As responsabilidades sao divididas entre Paineis rotativos (Infraestrutura,
  Moderacao, Tesouraria, ...). O Painel de Infraestrutura e o dono das aprovacoes de
  Los accesorios hacen SoraFS.
- **Sorteio e rotacao** - As cadeiras de Painel sao redesenhadas na cadencia definida na
  constituicao do Parlamento para que nenhum grupo monopolice como aprovacoes.

## Flujo de aprobación de accesorios1. **Presentación de propuesta**
   - O Tooling WG envió el paquete candidato `manifest_blake3.json` más la diferencia del accesorio
     para o registro en cadena a través de `sorafs.fixtureProposal`.
   - A propuesta registra o digest BLAKE3, a versao semantica e as notas de mudanca.
2. **Revisao e votacao**
   - O Painel de Infraestrutura recibe atribuicao pela fila de tarefas do Parlamento.
   - Miembros del dolor inspeccionam artefatos de CI, rodam testes de paridade e
     registram votos ponderados en cadena.
3. **Finalizacao**
   - Cuando el quórum y el tiempo de ejecución emiten un evento de aprobación que incluye
     resumen canónico del manifiesto y compromiso de Merkle de la carga útil del accesorio.
   - O evento e espelhado no registro SoraFS para que los clientes puedan buscar o
     manifiesto más reciente aprobado por el Parlamento.
4. **Distribución**
   - Helpers de CLI (`cargo xtask sorafs-fetch-fixture`) puxam o manifest aprovado via
     NexusRPC. Como constantes JSON/TS/Go do repositorio ficam sincronizadas ao
     reejecutar `export_vectors` y validar el resumen contra el registro en cadena.

## Flujo de trabajo de desarrollador

- Calendario de Regenere con:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```- Utilice el ayudante de buscar el Parlamento para bajar el sobre aprobado, verificar
  assinaturas e actualizar accesorios locales. Aponte `--signatures` para sobre
  publicado pelo Parlamento; o ayudante de resolución o manifiesto asociado, recomputa o
  digerir BLAKE3 e imponer el perfil canónico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Pase `--manifest` para obtener el manifiesto en otra URL. Sobres sin assinatura
sao recusados, a menos que `--allow-unsigned` seja definido para smoke run locais.

- Para validar un manifiesto a través de la puerta de enlace de staging, apote para Torii en vez de
  ubicaciones de cargas útiles:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- El CI local no exige más una lista `signer.json`.
  `ci/check_sorafs_fixtures.sh` compara el estado del repositorio con el último compromiso
  en cadena y falla cuando divergem.

## Notas de gobierno

- La constitución del Parlamento gobierna el quórum, la rotación y el escalamiento - nao e
  Necessaria configuracao no nivel do crate.
- Rollbacks de emergencia sao tratados pelo Painel de moderacao do Parlamento. oh
  Painel de Infraestrutura abre una propuesta de revertir que referencia o digest
  anterior se manifiesta, sustituyendo una liberación cuando se aprueba.
- Aprovacoes historicas permanecenm disponiveis no registra SoraFS para reproducir
  forense.

## Preguntas frecuentes- **Para onde foi `signer.json`?**  
  Foi removido. Toda la atribuicao de assinaturas vive en cadena; `manifest_signatures.json`
  no hay repositorio y sólo un accesorio de desarrollador que debe corresponder al último
  evento de aprobación.

- **¿Ainda exigimos assinaturas Ed25519 locais?**  
  Nao. As aprovacoes do Parlamento sao armazenadas como artefatos on-chain. Calendario
  Los lugares existen para la reproductibilidad, pero están validados contra el resumen del Parlamento.

- **¿Como equipos monitoram aprovacoes?**  
  Asigne el evento `ParliamentFixtureApproved` o consulte el registro a través de Nexus RPC
  para recuperar o digerir el actual do manifest e a chamada do Painel.