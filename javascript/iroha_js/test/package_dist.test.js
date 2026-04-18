"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { AccountAddress } from "../dist/index.js";

const LEGACY_FULLWIDTH_KANA = /[イロハニホヘトチリヌルヲワカヨタレソツネナラムウノオクヤマケフコエテアサキユメミシヒモセス]/u;
const HALFWIDTH_KANA = /[ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼﾋﾓｾｽ]/u;

test("package dist entrypoint imports and emits halfwidth i105 literals", () => {
  const publicKey = Buffer.from(
    "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
    "hex",
  );
  const address = AccountAddress.fromAccount({ publicKey });
  const literal = address.toI105(0x02f1);

  assert.match(literal, /^sora/u);
  assert.equal(LEGACY_FULLWIDTH_KANA.test(literal), false);
  assert.equal(HALFWIDTH_KANA.test(literal), true);
});
