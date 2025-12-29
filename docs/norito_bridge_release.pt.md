---
lang: pt
direction: ltr
source: docs/norito_bridge_release.md
status: complete
translator: manual
source_hash: bc7c766ff5fb0504f4da43a017bf294758800b9c815affc8f97b9bcc94ae8e15
source_last_modified: "2025-11-02T04:40:28.805628+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução para português de docs/norito_bridge_release.md (NoritoBridge Release Packaging) -->

# Empacotamento de Release do NoritoBridge

Este guia descreve os passos necessários para publicar os bindings Swift de
`NoritoBridge` como um XCFramework que pode ser consumido via Swift Package Manager e
CocoaPods. O workflow mantém os artefatos Swift em lock‑step com os releases do crate
Rust que fornece o codec Norito do Iroha. Para instruções ponta a ponta sobre como
consumir os artefatos publicados em um app (configuração do projeto Xcode, uso de
ChaChaPoly, etc.), consulte `docs/connect_swift_integration.md`.

> **Nota:** A automação de CI para este fluxo será adicionada assim que tivermos builders
> macOS com o tooling da Apple necessário (item rastreado no backlog de builders macOS da
> equipe de Release Engineering). Até lá, os passos abaixo devem ser executados
> manualmente em um Mac de desenvolvimento.

## Pré‑requisitos

- Host macOS com as Xcode Command Line Tools estáveis mais recentes instaladas.
- Toolchain Rust compatível com `rust-toolchain.toml` na raiz do workspace.
- Toolchain Swift 5.7 ou mais recente.
- CocoaPods (via Ruby gems) se for publicar no repositório central de specs.
- Acesso às chaves de assinatura de releases do Hyperledger Iroha para etiquetar os
  artefatos Swift.

## Modelo de versionamento

1. Determine a versão do crate Rust para o codec Norito (`crates/norito/Cargo.toml`).
2. Marque o workspace com o identificador de release (por exemplo, `v2.1.0`).
3. Use a mesma versão semântica para o pacote Swift e o podspec CocoaPods.
4. Quando a versão do crate Rust for incrementada, repita o processo e publique um
   artefato Swift equivalente. Versões podem incluir sufixos de metadata (por exemplo,
   `-alpha.1`) durante testes.

## Passos de build

1. A partir da raiz do repositório, invoque o script helper para montar o XCFramework:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   O script compila a biblioteca bridge em Rust para os targets iOS e macOS e agrupa as
   bibliotecas estáticas resultantes em um único diretório XCFramework.

2. Compacte o XCFramework para distribuição:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Atualize o manifest do pacote Swift (`IrohaSwift/Package.swift`) para apontar para a
   nova versão e checksum:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   Registre o checksum em `Package.swift` ao definir o binary target.

4. Atualize `IrohaSwift/IrohaSwift.podspec` com a nova versão, checksum e URL do
   arquivo.

5. **Regenere os headers se o bridge passou a exportar novos símbolos.** O bridge Swift
   agora expõe `connect_norito_set_acceleration_config` para que `AccelerationSettings`
   possa alternar backends Metal/GPU. Certifique‑se de que
   `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` esteja em sintonia com
   `crates/connect_norito_bridge/include/connect_norito_bridge.h` antes de gerar o zip.

6. Rode a suíte de validação Swift antes de taguear:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   O primeiro comando verifica se o pacote Swift (incluindo `AccelerationSettings`)
   continua compilando sem erros; o segundo valida a paridade de fixtures, renderiza os
   dashboards de paridade/CI e exerce as mesmas verificações de telemetria exigidas no
   Buildkite (incluindo o metadata `ci/xcframework-smoke:<lane>:device_tag`).

7. Faça commit dos artefatos gerados em uma branch de release e crie uma tag no commit.

## Publicação

### Swift Package Manager

- Envie a tag para o repositório Git público.
- Verifique se a tag é visível para o índice de pacotes (Apple ou espelho da comunidade).
- Consumidores podem depender de
  `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`.

### CocoaPods

1. Valide o pod localmente:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Envie o podspec atualizado:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. Confirme que a nova versão aparece no índice do CocoaPods.

## Considerações de CI

- Crie um job macOS que execute o script de empacotamento, arquive os artefatos e envie o
  checksum gerado como output do workflow.
- Condicione os releases ao build bem‑sucedido do app demo em Swift contra o framework
  recém‑gerado.
- Armazene logs de build para facilitar diagnósticos de falhas.

## Ideias extras de automação

- Usar `xcodebuild -create-xcframework` diretamente, assim que todos os targets
  necessários estiverem expostos.
- Integrar assinatura/notarização para distribuição fora de máquinas de desenvolvimento.
- Manter testes de integração em lock‑step com a versão empacotada, fixando a dependência
  SPM na tag de release.

