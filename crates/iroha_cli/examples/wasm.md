# Examples for `iroha wasm`

In this section we will show you how to use Iroha CLI Wasm to do the following:

  - [Check smartcontracts](#check-smartcontract)
  - [Build smartcontracts](#build-smartcontracts)
  
## Check smartcontract

```bash
./iroha wasm check path/to/project
```

## Build smartcontracts

```bash
./iroha wasm build path/to/project --out-file ./smartcontract.wasm
```

**Build with options:**

```bash
./iroha wasm build path/to/project --optimize --format --out-file ./smartcontract.wasm
```

## Test WebAssembly
This command copies functionality of `webassembly-test-runner`, but with an ability to indicate failure with an exit code.