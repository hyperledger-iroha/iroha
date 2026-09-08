import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { validateNoritoFrame } from '../src/norito.js';
import { encodeGameValueV1, decodeGameValueV1, encodeGameResourceValueV1, decodeGameResourceValueV1 } from '../src/game.js';
import { encodeNftMarketValueV1, decodeNftMarketValueV1 } from '../src/nft.js';

// Required native-origin exporter output. Missing fixtures fail; JS never generates goldens.
const fixture = JSON.parse(readFileSync(new URL('fixtures/game-resource-v1-codec.json', import.meta.url), 'utf8'));
assert.equal(fixture.version, 1);
assert.equal(fixture.vectors.length, 16);
for (const row of fixture.vectors) test(`native equipment ${row.name}/${row.case} exact canonical bytes`, () => {
  const name = row.name === 'VecGameResourceReservationRecordV1' ? 'records' : row.name;
  const [encode, decode] = row.name.startsWith('Nft') ? [encodeNftMarketValueV1, decodeNftMarketValueV1]
    : row.name.startsWith('GameResource') || name === 'records' ? [encodeGameResourceValueV1, decodeGameResourceValueV1]
    : [encodeGameValueV1, decodeGameValueV1];
  const bytes = encode(name, row.value);
  assert.equal(Buffer.from(bytes).toString('hex').toUpperCase(), row.encoded_hex);
  const framed = Buffer.from(row.framed_hex, 'hex');
  const frame = validateNoritoFrame(framed, { expectedPaddingLength: 0, requireNonEmptyPayload: true });
  assert.equal(frame.flags, row.name === 'NftCustodyPurposeV1' ? 0 : 2, 'fixed unit enums use no length flags; records use native compact lengths');
  assert.deepEqual(frame.payload, Buffer.from(bytes), 'native framed payload equals SDK bare bytes');
  const badCrc = Buffer.from(framed); badCrc[31] ^= 1;
  assert.throws(() => validateNoritoFrame(badCrc), /CRC|checksum/i);
  const badSchema = Buffer.from(framed); badSchema[6] ^= 1;
  assert.throws(() => validateNoritoFrame(badSchema, { expectedSchemaHash: frame.schemaHash }), /schema/);
  assert.deepEqual(Buffer.from(encode(name, decode(name, Buffer.from(row.encoded_hex, 'hex')))), Buffer.from(bytes));
  assert.throws(() => decode(name, Buffer.concat([Buffer.from(bytes), Buffer.of(0)])));
  assert.throws(() => decode(name, Buffer.from(row.framed_hex, 'hex')), 'bare codec must not silently accept a framed native value');
});
