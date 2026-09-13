"use strict";

// Run through sccp_evm_contract_smoke.sh after the audited native runtime install.
const assert = require("node:assert/strict");
const test = require("node:test");
const { createNativeEdrProvider } = require("../contract_tooling/evm-runtime/edr-provider.js");
const { rejectedWith } = require("../contract_tooling/evm-runtime/evm-errors.js");
const { AbiCoder } = require("ethers");
const { BrowserProvider } = require("ethers");

test("revert assertions accept exact native bytes and reject unrelated RPC failures", () => {
  const encoded = `0x08c379a0${AbiCoder.defaultAbiCoder().encode(["string"], ["SC_BREAKER"]).slice(2)}`;
  const native = { code: -32000, data: { data: encoded, reason: { Revert: encoded } } };
  assert(rejectedWith("SC_BREAKER")({ error: native }));
  assert(rejectedWith("SC_BREAKER")({ code: "CALL_EXCEPTION", data: encoded }));
  assert(rejectedWith()(native));
  assert(!rejectedWith("other")(native));
  assert(!rejectedWith("SC_BREAKER")({ code: -32000, message: "SC_BREAKER revert" }));
  assert(!rejectedWith()({ code: -32000, data: { data: encoded, reason: { OutOfGas: encoded } } }));
  assert(!rejectedWith()({ code: -32000, data: { data: encoded, reason: { Revert: "0x" } } }));
  assert(!rejectedWith("SC_BREAKER")({ code: "CALL_EXCEPTION", data: `${encoded}00` }));
  assert(!rejectedWith()({ code: "CALL_EXCEPTION", data: "0x1" }));
});

test("native EDR rejects malformed limits and requests", async () => {
  assert.throws(() => createNativeEdrProvider({ chainId: 0, blockGasLimit: 20_000_000 }));
  assert.throws(() => createNativeEdrProvider({ chainId: 1, blockGasLimit: NaN }));
  const provider = createNativeEdrProvider({ chainId: 56, blockGasLimit: 20_000_000 });
  try {
    await assert.rejects(provider.request({ method: "eth_chainId", params: {} }));
    await assert.rejects(provider.request({ method: "unavailable_sccp_method" }),
      (error) => Number.isInteger(error.code));
  } finally {
    await provider.disconnect();
  }
  await assert.rejects(provider.request({ method: "eth_chainId" }), /disconnected/);
});

test("native EDR keeps chain identity, balances and mined state isolated", async () => {
  const first = createNativeEdrProvider({ chainId: 56, blockGasLimit: 20_000_000 });
  const second = createNativeEdrProvider({ chainId: 1, blockGasLimit: 20_000_000 });
  try {
    assert.equal(await first.request({ method: "eth_chainId" }), "0x38");
    assert.equal(await second.request({ method: "eth_chainId" }), "0x1");
    const [sender, recipient] = await first.request({ method: "eth_accounts" });
    const initial = await second.request({ method: "eth_getBalance", params: [recipient, "latest"] });
    const hash = await first.request({ method: "eth_sendTransaction", params: [{ from: sender, to: recipient, value: "0x123" }] });
    const receipt = await first.request({ method: "eth_getTransactionReceipt", params: [hash] });
    assert.equal(receipt.status, "0x1");
    assert.equal(await second.request({ method: "eth_getBalance", params: [recipient, "latest"] }), initial);
    assert.equal(BigInt(await first.request({ method: "eth_getBalance", params: [recipient, "latest"] })), BigInt(initial) + 0x123n);
    assert.equal(await first.request({ method: "eth_blockNumber" }), "0x1");
    assert.equal(await second.request({ method: "eth_blockNumber" }), "0x0");
  } finally {
    await first.disconnect();
    await second.disconnect();
  }
});

test("native EDR reports mined reverts and enforces the contract-size ceiling", async () => {
  const provider = createNativeEdrProvider({ chainId: 56, blockGasLimit: 20_000_000 });
  try {
    const [sender] = await provider.request({ method: "eth_accounts" });
    await assert.rejects(provider.request({ method: "eth_sendTransaction", params: [{
      from: sender, data: "0x60006000fd", gas: "0x186a0",
    }] }), (error) => Number.isInteger(error.code) && /revert/i.test(error.message));
    assert.equal(await provider.request({ method: "eth_getTransactionCount", params: [sender, "latest"] }), "0x1");
    assert.equal(await provider.request({ method: "eth_blockNumber" }), "0x1");
    const oversized = `0x61600161000f6000396160016000f3${"00".repeat(24_577)}`;
    await assert.rejects(provider.request({ method: "eth_sendTransaction", params: [{
      from: sender, data: oversized, gas: "0xf42400",
    }] }), (error) => Number.isInteger(error.code) && /code.*(large|size)/i.test(error.message));
  } finally {
    await provider.disconnect();
  }
});

test("uncached native state reads observe changed revert reasons immediately", async () => {
  const provider = createNativeEdrProvider({ chainId: 56, blockGasLimit: 20_000_000 });
  const ethereum = new BrowserProvider(provider, undefined, { cacheTimeout: -1 });
  try {
    const signer = await ethereum.getSigner();
    const address = "0x0000000000000000000000000000000000000123";
    for (const reason of ["SC_REPLAY", "SC_BREAKER"]) {
      const encoded = `08c379a0${AbiCoder.defaultAbiCoder().encode(["string"], [reason]).slice(2)}`;
      const length = (encoded.length / 2).toString(16).padStart(2, "0");
      const code = `0x60${length}600c60003960${length}6000fd${encoded}`;
      await provider.request({ method: "hardhat_setCode", params: [address, code] });
      await assert.rejects(signer.estimateGas({ to: address, data: "0x1234" }), rejectedWith(reason));
    }
  } finally {
    await provider.disconnect();
  }
});
