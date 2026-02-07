---
lang: es
direction: ltr
source: docs/portal/docs/norito/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visao general de Norito

Norito y una camada de serializacao binaria usada en todo o Iroha: define como estructuras de datos sao codificadas en la red, persistidas en disco y trocadas entre contratos y hosts. Cada caja sin espacio de trabajo depende de Norito en vez de `serde` para que pares en hardware produzcan bytes idénticos.

Esta visao general resume as partes principais e aponta para as referencias canónicas.

## Arquitectura en currículum

- **Cabecalho + payload** - Cada mensaje Norito viene con un cabecalho de negociación de características (flags, checksum) seguido de payload puro. Diseños empacotados y comprimidos sao negociados vía bits do cabecalho.
- **Codificación determinística** - `norito::codec::{Encode, Decode}` implementa una base de codificación. El mismo diseño y reutilizado para involucrar cargas útiles en cabecalhos para que el hash y la assinatura sigan determinísticas.
- **Esquema + derivaciones** - `norito_derive` es una implementación de `Encode`, `Decode` e `IntoSchema`. Estructuras/secuencias empacotadas sao activadas por padrao e documentadas en `norito.md`.
- **Registro multicodec** - Identificadores de hashes, tipos de chave escriptres de payload viven en `norito::multicodec`. Una tabla de referencia fica en `multicodec.md`.

## Ferramentas| Tarefa | Comando/API | Notas |
| --- | --- | --- |
| Inspeccionar cabecalho/secoes | `ivm_tool inspect <file>.to` | Mostrao verso de ABI, banderas y puntos de entrada. |
| Codificar/decodificar en Rust | `norito::codec::{Encode, Decode}` | Implementado para todos los tipos principales del modelo de datos. |
| JSON de interoperabilidad | `norito::json::{to_json_pretty, from_json}` | JSON determinístico apoiado por valores Norito. |
| Gerar documentos/específicos | `norito.md`, `multicodec.md` | Documentacao fonte de verdade na raiz do repo. |

## Flujo de trabajo de desenvolvimento

1. **Agregar derivas** - Prefira `#[derive(Encode, Decode, IntoSchema)]` para nuevas estructuras de dados. Evite serializadores feitos a mao salvo se for absolutamente necesario.
2. **Validar diseños empacotados** - Utilice `cargo test -p norito` (y una matriz de características empacotadas en `scripts/run_norito_feature_matrix.sh`) para garantizar que los nuevos diseños estén fijos.
3. **Regenerar documentos** - Cuando se cambia la codificación, se actualiza `norito.md` y una tabla multicodec, se actualizan las páginas del portal (`/reference/norito-codec` y esta visa general).
4. **Manter testes Norito-first** - Testes de integracao devem usar os helpers JSON do Norito em vez de `serde_json` para ejercitar os mesmos caminhos da producao.

## Enlaces rapidos

- Específica: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Atribuicos multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matriz de características: `scripts/run_norito_feature_matrix.sh`
- Ejemplos de diseño empacotado: `crates/norito/tests/`Combine esta visa general con la guía de inicio rápido (`/norito/getting-started`) para un paso práctico de compilar y ejecutar el código de bytes que usa las cargas útiles Norito.