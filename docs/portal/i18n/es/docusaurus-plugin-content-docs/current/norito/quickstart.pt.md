---
lang: es
direction: ltr
source: docs/portal/docs/norito/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Inicio rápido de Norito
descripción: Construya, valide e faca implementar de un contrato Kotodama con o herramientas de liberación y una red padrao de un único par.
babosa: /norito/inicio rápido
---

Este paso a paso espelha o flujo que esperamos que desenvolvedores sigan ao aprender Norito e Kotodama pela primera vez: iniciar una red determinística de un único par, compilar un contrato, hacer un funcionamiento en seco localmente y enviarlo a través de Torii con o CLI de referencia.

O contrato de exemplo grava um par chave/valor na conta do chamador para que voce possa verificar o efeito colateral inmediatamente com `iroha_cli`.

##Requisitos previos

- [Docker](https://docs.docker.com/engine/install/) con Compose V2 habilitado (usado para iniciar o par de ejemplo definido en `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) para compilar los binarios auxiliares y descargar los publicados.
- Binarios `koto_compile`, `ivm_run` e `iroha_cli`. Puedes compilar los archivos a partir del checkout del espacio de trabajo como mostrar abajo o bajar los artefactos de lanzamiento correspondientes:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Os binarios acima sao seguros para instalar junto con el resto del espacio de trabajo.
> Eles nunca fazem link com `serde`/`serde_json`; os codecs Norito sao aplicaciones de ponta a ponta.

## 1. Inicie uma rede dev de um unico peerEl repositorio incluye un paquete Docker Compose generado por `kagami swarm` (`defaults/docker-compose.single.yml`). Se conecta a genesis padrao, a configuración del cliente y a las sondas de salud para que Torii fique acessivel em `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deixe o contenedor rodando (em primeiro plano ou detached). Todas las chamadas de CLI posteriores apontam para esse peer vía `defaults/client.toml`.

## 2. Escreva o contrato

Llame a un directorio de trabajo y salve el ejemplo mínimo de Kotodama:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Prefira mantener las fuentes Kotodama en control de versao. Exemplos hospedados no portal tambem thiso disponiveis na [galeria de exemplos Norito](./examples/) se voce quiser um ponto de partida mais rico.

## 3. Compilar y ejecutar en seco con IVM

Compile el contrato para el código de bytes IVM/Norito (`.to`) y ejecútelo localmente para confirmar que las llamadas al sistema del host funcionan antes de tocar una red:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

El corredor imprime el registro `info("Hello from Kotodama")` y ejecuta la llamada al sistema `SET_ACCOUNT_DETAIL` contra el host simulado. Se o binario opcional `ivm_tool` estiver disponivel, `ivm_tool inspect target/quickstart/hello.to` muestra el encabezado ABI, los bits de características y los puntos de entrada exportados.

## 4. Envíe el código de bytes a través de ToriiComo nodo todavía rodando, envie el código de bytes compilado para Torii usando la CLI. A identidade desenvolvimento padrao e derivada da chave publica em `defaults/client.toml`, portanto o ID de conta e
```
<katakana-i105-account-id>
```

Utilice el archivo de configuración para fornecer una URL de Torii, el ID de cadena y una clave de configuración:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

La CLI codifica a transacao con Norito, assina com a chave de dev e envia ao peer em execucao. Observe los registros de Docker para la llamada al sistema `set_account_detail` o monitoree la CLI para el hash de la transacción comprometida.

## 5. Verifique a mudanca de estado

Utilice el mismo perfil de CLI para buscar el detalle de la cuenta que o contrato grave:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <katakana-i105-account-id> \
  --key example | jq .
```

Debes ver la carga útil JSON compatible con Norito:

```json
{
  "hello": "world"
}
```

Se o valor estiver ausente, confirme que o servico Docker compose ainda esta rodando e que o hash da transacao reportado por `iroha` chegou ao estado `Committed`.

## Próximos pasos- Explora una [galería de ejemplos](./examples/) gerada automáticamente para ver
  como fragmentos Kotodama pero avancados se mapean para syscalls Norito.
- Leia o [guía de inicio de Norito](./getting-started) para una explicación
  Más profundas son las herramientas del compilador/runner, la implementación de manifiestos y dos metadados IVM.
- Para iterar nuestros contratos, utilice `npm run sync-norito-snippets` sin espacio de trabajo para
  Regenerar fragmentos bajos y mantener los documentos del portal y artefactos sincronizados con fuentes en `crates/ivm/docs/examples/`.