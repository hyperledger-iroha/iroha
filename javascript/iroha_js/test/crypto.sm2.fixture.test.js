import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import { sm2FixtureFromSeed } from "../src/crypto.js";
import { makeNativeTest } from "./helpers/native.js";

const DISTID = "iroha-sdk-sm2-fixture";
const SEED = Buffer.from("iroha-rust-sdk-sm2-deterministic-fixture");
const MESSAGE = Buffer.from("Rust SDK SM2 signing fixture v1");

const testNative = makeNativeTest(baseTest);

testNative("sm2 fixture matches rust reference", () => {
  const fixture = sm2FixtureFromSeed(DISTID, SEED, MESSAGE);
  assert.equal(fixture.distid, DISTID);
  assert.equal(fixture.seedHex, Buffer.from(SEED).toString("hex").toUpperCase());
  assert.equal(fixture.messageHex, Buffer.from(MESSAGE).toString("hex").toUpperCase());
  assert.equal(
    fixture.privateKeyHex,
    "E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA",
  );
  assert.equal(
    fixture.publicKeySec1Hex,
    "0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61",
  );
  assert.equal(
    fixture.publicKeyMultihash,
    "862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61",
  );
  assert.equal(
    fixture.publicKeyPrefixed,
    "sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61",
  );
  assert.equal(fixture.za, "6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE");
  assert.equal(
    fixture.signature,
    "4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5",
  );
  assert.equal(fixture.r, "4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76");
  assert.equal(fixture.s, "299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5");
});
