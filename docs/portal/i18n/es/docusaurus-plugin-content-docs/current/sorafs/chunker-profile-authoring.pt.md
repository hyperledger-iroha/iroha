---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: creación de perfiles fragmentados
título: Guía de autoria de perfis de chunker da SoraFS
sidebar_label: Guía de autoridad de chunker
descripción: Lista de verificación para proporcionar nuevos daños y accesorios de fragmentador de SoraFS.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/chunker_profile_authoring.md`. Mantenha ambas como copias sincronizadas.
:::

# Guía de autoria de perfis de chunker da SoraFS

Esta guía explica cómo proporcionar y publicar novos perfis de chunker para SoraFS.
Ele complementa el RFC de arquitectura (SF-1) y la referencia del registro (SF-2a)
com requisitos concretos de autoria, etapas de validación y modelos de propuesta.
Para un ejemplo canónico, veja
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o log de dry-run asociado em
`docs/source/sorafs/reports/sf1_determinism.md`.

## Visao general

Cada perfil que ingresa no registro debe:

- anunciar parámetros determinísticos CDC y configuraciones de multihash idénticos entre
  arquitecturas;
- Entregar accesorios reproducidos (JSON Rust/Go/TS + corpora fuzz + testemunhas PoR) que
  Los SDK posteriores pueden verificar las herramientas en función de la medida;
- incluir metadados prontos paragobernanza (namespace, name, semver) junto com orientacao
  de despliegue y janelas operativas; mi
- pasar pela suite de diff determinista antes de la revisión del consejo.

Siga una lista de verificación a continuación para preparar una propuesta que atenda a essas regras.

## Resumen de la carta del registroAntes de redigir una propuesta, confirme que ella asiste a la carta de registro aplicada por
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- ID de perfil sao inteiros positivos que aumentan de forma monotona sin lagunas.
- O handle canonico (`namespace.name@semver`) debe aparecer en la lista de alias e
  **deve** ser la primera entrada. Alias ​​alternativos (ej., `sorafs.sf1@1.0.0`) vem después.
- Nenhum alias pode colidir com outro handle canonico ou aparecer mais de una vez.
- Los alias deben ser nao vazios e aparados de espacos em branco.

Ayudantes de CLI:

```bash
# Listagem JSON de todos os descritores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadados para um perfil default candidato (handle canonico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Estos comandos mantem as propostas alinhadas com a carta do registro e fornecem os
metadados canónicos necesarios nas discusiones de gobierno.

## Metadados requeridos| Campo | Descripción | Ejemplo (`sorafs.sf1@1.0.0`) |
|-------|-----------|------------------------------|
| `namespace` | Agrupamento lógico para perfis relacionados. | `sorafs` |
| `name` | Rotulo legivel para humanos. | `sf1` |
| `semver` | Cadeia de versao semántica para el conjunto de parámetros. | `1.0.0` |
| `profile_id` | Identificador numérico monótono atribuido cuando o perfil entra. Reserve o proximo id mas nao reutilice numeros existentes. | `1` |
| `profile_aliases` | Maneja adicionais opcionais (nombres alternativos, abreviacoes) exposiciones a clientes durante una negociación. Inclua siempre o handle canonico como primeira entrada. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Compromiso mínimo de fragmentar bytes. | `65536` |
| `profile.target_size` | Compromiso también de fragmentar los bytes. | `262144` |
| `profile.max_size` | Comprimento maximo do fragment em bytes. | `524288` |
| `profile.break_mask` | Mascara adaptativa usada pelo Rolling Hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Engranaje constante de polinomio (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Semilla usada para derivar a tabla de engranajes de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para compendio por fragmento. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Resumen del paquete canónico de accesorios. | `13fa...c482` || `fixtures_root` | Diretorio relativo contendo os fixtures regenerados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semilla para amostragem PoR determinística (`splitmix64`). | `0xfeedbeefcafebabe` (ejemplo) |

Los metadados deben aparecer tanto en ningún documento de propuesta cuanto dentro de los accesorios generados.
para que el registro, las herramientas de CLI y el control automático confirmen los valores sem
cruzamentos manuales. En caso de duda, ejecute las CLI de chunk-store y manifest com
`--json-out=-` para transmitir os metadados calculados para notas de revisión.

### Puntos de contacto de CLI y registro

- `sorafs_manifest_chunk_store --profile=<handle>` - reejecutar metadados de chunk,
  resumen del manifiesto y comprobaciones PoR con los parámetros propuestos.
- `sorafs_manifest_chunk_store --json-out=-` - transmitir o relatorio do chunk-store para
  stdout para comparaciones automáticas.
- `sorafs_manifest_stub --chunker-profile=<handle>` - confirmar que manifiestos y planos CAR
  embutem o manejar canonico mais alias.
- `sorafs_manifest_stub --plan=-` - reenviar o `chunk_fetch_specs` anterior para
  verificar compensaciones/resúmenes apos a mudanca.

Registre a sayda dos comandos (digests, raizes PoR, hashes de manifest) na propuesta para que
os revisores possam reproduzi-los literalmente.

## Lista de verificación de determinismo y validación1. **Regenerar accesorios**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Ejecutar una suite de paridade** - `cargo test -p sorafs_chunker` e o Harness Diff
   lenguaje cruzado (`crates/sorafs_chunker/tests/vectors.rs`) devem ficar verdes com os
   novos accesorios no lugar.
3. **Reejecutar corpora fuzz/back-pression** - ejecute `cargo fuzz list` y el arnés de
   streaming (`fuzz/sorafs_chunker`) contra los activos regenerados.
4. **Verificar testemunhas Prueba de recuperación** - ejecutar
   `sorafs_manifest_chunk_store --por-sample=<n>` usando el perfil propuesto y confirmado
   que as raizes correspondenm ao manifest de fixtures.
5. **Ejecución en seco de CI** - ejecute `ci/check_sorafs_fixtures.sh` localmente; o guión
   Debe tener éxito con los nuevos accesorios y el `manifest_signatures.json` existente.
6. **Confirmación entre tiempos de ejecución**: asegúrese de que los enlaces Go/TS consuman o JSON
   regenerado e emitam limites e digests identicos.

Documente los comandos y los resúmenes resultantes de la propuesta para que el Tooling WG pueda
reexecuta-los sem adivinhacoes.

### Confirmación de manifiesto / PoR

Después de regenerar accesorios, ejecute o pipeline completo de manifest para garantizar que
metadados CAR y pruebas PoR continúan consistentes:

```bash
# Validar metadados de chunk + PoR com o novo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Gerar manifest + CAR e capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reexecutar usando o plano de fetch salvo (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Sustitua o archivo de entrada por cualquier corpus representativo usado en nuestros accesorios
(ej., una secuencia determinística de 1 GiB) y un anexo de los resúmenes resultantes a propuesta.

## Modelo de propuestaAs propostas sao submetidas como registros Norito `ChunkerProfileProposalV1` registrados en
`docs/source/sorafs/proposals/`. Plantilla JSON a continuación ilustra el formato esperado
(sustitua sus valores conforme necesario):


Forneca um relatorio Markdown corresponsal (`determinism_report`) que captura un
dijo dos comandos, resúmenes de fragmentos y quaisquer desvios encontrados durante una validación.

## Flujo de gobierno

1. **Submeter PR con propuesta + accesorios.** Incluye los activos gerados, una propuesta
   Norito y actualizado en `chunker_registry_data.rs`.
2. **Revisao do Tooling WG.** Revisores reexecutam a checklist de validacao e confirmam
   que a propuesta segue as regras do registro (sem reutilizacao de id, determinismo satisfeito).
3. **Sobre do conselho.** Uma vez aprobado, membros do conselho assinam o digest da
   propuesta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) y anexam suas
   assinaturas ao envolvente do perfil armazenado junto aos accesorios.
4. **Publicacao do registro.** O merge atualiza el registro, documentos y accesorios. O CLI
   default permanece sin perfil anterior ate que un gobierno declare una migración inmediata.
5. **Rastreamento de deprecacao.** Apos a janela de migracao, atualize o registro para

## Dicas de autoria- Prefira limitar los pares de potencia de dos para minimizar el comportamiento de fragmentación en los bordes.
- Evite cambiar el código multihash sin coordinar a los consumidores de manifest y gateway; incluye una
  nota operativa cuando fizer isso.
- Mantenha as seeds da tabela gear legiveis para humanos, mas globalmente unicas para simplificar auditorias.
- Armazene artefatos de benchmarking (ej., comparaciones de rendimiento) em
  `docs/source/sorafs/reports/` para referencia futura.

Para expectativas operativas durante el lanzamiento, consulte el libro mayor de migración.
(`docs/source/sorafs/migration_ledger.md`). Para registros de conformidad en tiempo de ejecución, veja
`docs/source/sorafs/chunker_conformance.md`.