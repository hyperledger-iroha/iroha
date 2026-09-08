import test from "node:test";
import assert from "node:assert/strict";
import { ed25519 } from "@noble/curves/ed25519";
import { AccountAddress } from "../src/address.js";
import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";
import { encodeGameResourceValueV1 as encode, decodeGameResourceValueV1 as decode,
  validateGameResourceClausesV1 as clauses, validateGameResourceRequirementsV1 as requirements,
  matchGameResourceRequirementsV1 as match, validateGameResourceReservationSetV1 as retained,
  GAME_RESOURCE_MAX_SET_BYTES_V1, GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1 } from "../src/gameResources.js";

// These construction vectors originate in JavaScript. They exercise SDK contracts
// and rejection, not native whole-value wire parity or deployed custody behavior.
const hash = n => { const bytes = Buffer.alloc(32, n); bytes[31] |= 1; const body = bytes.toString("hex").toUpperCase(); return `hash:${body}#${computeHashLiteralCrc("hash", body)}`; };
const accounts = new Map();
const account = n => {
  if (!accounts.has(n)) accounts.set(n, AccountAddress.fromAccount({ algorithm: "ed25519", publicKey: ed25519.getPublicKey(new Uint8Array(32).fill(n)) }).toI105());
  return accounts.get(n);
};
const policy = () => ({ kind: "return_to_original_owner_at_terminal", value: null });
const clause = n => ({ nft_id: `kit${n}$equipment.universal`, expected_metadata_hash: hash(9), role_id: hash(n), policy: policy() });
const set = () => ({ version: 1, network_id: hash(17), session_id: hash(19), records: [0, 1].map(slot => ({
  slot, nft_id: clause(slot + 1).nft_id, metadata_hash: hash(9), role_id: hash(slot + 1), policy: policy(),
  original_owner: account(slot + 1), custody: account(slot + 41), reserved_at_height: String(10 + slot), released_at_height: null,
})) });

test("generic SDK resource values roundtrip with exact explicit authorization and no input mutation", () => {
  const input = [clause(1), clause(2)], before = structuredClone(input);
  assert.deepEqual(clauses(input), input); assert.deepEqual(requirements(input), input);
  assert.equal(match(input, input), undefined);
  assert.deepEqual(encode("clauses", input), encode("requirements", input));
  assert.notStrictEqual(clauses(input)[0].policy, input[0].policy);
  for (const [name, value] of [["GameResourceReturnPolicyV1", policy()], ["GameResourceReservationClauseV1", clause(1)],
    ["GameResourceRequirementV1", clause(1)], ["GameResourceReservationRecordV1", set().records[0]], ["GameResourceReservationSetV1", set()]]) {
    const bytes = encode(name, value);
    assert.deepEqual(decode(name, bytes), value);
    assert.throws(() => decode(name, Buffer.concat([bytes, Buffer.of(0)])), /trailing|unknown|eight bytes/);
  }
  assert.deepEqual(input, before);
});

test("adapter requirements cannot imply missing custody, excess authorizations or changed metadata", () => {
  const approved = [clause(1)], before = structuredClone(approved);
  for (const required of [[], [clause(1), clause(2)], [{ ...clause(1), expected_metadata_hash: hash(11) }],
    [{ ...clause(1), nft_id: clause(2).nft_id }], [{ ...clause(1), role_id: hash(3) }]]) {
    assert.throws(() => match(approved, required), /match/);
  }
  assert.throws(() => match([], approved), /match/);
  assert.deepEqual(approved, before);
});

test("closed return policy rejects wager/burn/recipient substitutions and unknown binary discriminants", () => {
  assert.deepEqual(encode("GameResourceReturnPolicyV1", policy()), Buffer.alloc(4));
  for (const value of [{ kind: "return_to_winner_at_terminal", value: null }, { kind: "burn", value: null },
    { kind: policy().kind }, { kind: policy().kind, value: account(2) }, { ...policy(), recipient: account(2) }]) {
    assert.throws(() => clauses([{ ...clause(1), policy: value }]), /policy|exact/);
  }
  for (const bytes of [Buffer.of(1, 0, 0, 0), Buffer.of(0), Buffer.alloc(5)]) {
    assert.throws(() => decode("GameResourceReturnPolicyV1", bytes), /policy/);
  }
  assert.throws(() => clauses([{ ...clause(1), application_data: [] }]), /exact fields/);
  assert.throws(() => encode("JoinGameSessionV2", {}), /unknown/);
});

test("bounded clauses reject duplicate NFT/roles, noncanonical order, identifiers and excess elements", () => {
  assert.equal(clauses([1, 2, 3, 4].map(clause)).length, 4);
  for (const values of [[clause(2), clause(1)], [clause(1), { ...clause(2), role_id: hash(1) }],
    [clause(1), { ...clause(2), nft_id: clause(1).nft_id }], [1, 2, 3, 4, 5].map(clause)]) assert.throws(() => clauses(values));
  for (const nft_id of ["kit$equipment", `${"x".repeat(513)}$equipment.universal`]) assert.throws(() => clauses([{ ...clause(1), nft_id }]), /canonical|string byte bound/);
  assert.throws(() => clauses([{ ...clause(1), role_id: hash(1).toLowerCase() }]), /canonical/);
});

test("plain exact objects and dense arrays reject hidden authorizations without invoking getters", () => {
  let reads = 0;
  const getter = { ...clause(1) }; Object.defineProperty(getter, "nft_id", { enumerable: true, get() { reads++; return clause(1).nft_id; } });
  const array = [clause(1)]; Object.defineProperty(array, "0", { enumerable: true, get() { reads++; return clause(1); } });
  const symbol = { ...clause(1), [Symbol("wager")]: true };
  const inherited = Object.assign(Object.create({ recipient: account(2) }), clause(1));
  for (const value of [[getter], array, [symbol], [inherited], new Array(1), Object.assign([clause(1)], { extra: true })]) assert.throws(() => clauses(value));
  const owners = [account(1), account(2)]; Object.defineProperty(owners, "0", { enumerable: true, get() { reads++; return account(1); } });
  assert.throws(() => retained(set(), owners), /data elements/);
  assert.equal(reads, 0);
});

test("retained resources reject aliases, wrong roster, duplicate slots and partial terminal returns", () => {
  const original = set(); assert.deepEqual(retained(original, [account(1), account(2)]), original);
  assert.throws(() => retained(original, [account(2), account(1)]), /original owners/);
  const unequippedAlias = structuredClone(original); unequippedAlias.records[0].custody = account(3);
  retained(unequippedAlias);
  assert.throws(() => retained(unequippedAlias, [account(1), account(2), account(3)]), /original owners/);
  const mutations = [s => s.records.reverse(), s => { s.records[1].nft_id = s.records[0].nft_id; },
    s => { s.records[1].custody = s.records[0].custody; }, s => { s.records[0].custody = s.records[1].original_owner; },
    s => { s.records[1].original_owner = s.records[0].original_owner; }, s => { s.records[0].released_at_height = "20"; },
    s => { s.records[0].reserved_at_height = "0"; }, s => { s.records[0].released_at_height = "9"; },
    s => { s.records[1].slot = 0; }, s => { s.records[1].slot = 32; }];
  for (const mutate of mutations) { const changed = structuredClone(original); mutate(changed); assert.throws(() => retained(changed)); }
  const returned = structuredClone(original); for (const row of returned.records) row.released_at_height = "20";
  assert.deepEqual(retained(returned), returned);
  returned.records[1].released_at_height = "21"; assert.throws(() => retained(returned), /atomically/);
  assert.throws(() => retained({ ...original, records: original.records.map(row => ({ ...row, recipient: account(2) })) }), /exact fields/);
});

test("all 32 permanent slots can retain four unique resources without relaxing ordering or count caps", () => {
  const saturated = { ...set(), records: [] }, owners = [];
  for (let slot = 0; slot < 32; slot++) {
    owners.push(account(slot + 1));
    for (let role = 1; role <= 4; role++) saturated.records.push({ ...set().records[0], slot, original_owner: account(slot + 1),
      nft_id: `kit${slot}_${role}$equipment.universal`, role_id: hash(role), custody: account(41 + slot * 4 + role - 1) });
  }
  assert.equal(retained(saturated, owners).records.length, 128);
  assert.ok(encode("GameResourceReservationSetV1", saturated).length < GAME_RESOURCE_MAX_SET_BYTES_V1);
  assert.throws(() => retained({ ...saturated, records: [...saturated.records, saturated.records[0]] }), /array bound/);
  const excessForSlot = { ...saturated, records: saturated.records.slice(0, 5).map((row, index) => ({ ...row, slot: 0, role_id: hash(index + 1), original_owner: account(1) })) };
  assert.throws(() => retained(excessForSlot), /resources per slot/);
});

test("exact integer and identifier limits reject rounding, alternate decimals and oversized input before parsing", () => {
  for (const reserved_at_height of [-1, 1.1, Number.MAX_SAFE_INTEGER + 1, "01", "18446744073709551616", "1".repeat(1_000)]) {
    const value = set(); value.records[0].reserved_at_height = reserved_at_height; assert.throws(() => retained(value));
  }
  const value = set(); value.records[0].reserved_at_height = "18446744073709551615";
  value.records.forEach(row => { row.released_at_height = "18446744073709551615"; });
  assert.equal(retained(value).records[0].reserved_at_height, "18446744073709551615");
  value.records[0].original_owner = "x".repeat(GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1 + 1);
  assert.throws(() => retained(value), /string byte bound/);
});

test("bare resource decoders enforce bounds before vector allocation and reject alternate framing", () => {
  const count = Buffer.alloc(8); count.writeBigUInt64LE(0xffff_ffff_ffff_ffffn);
  assert.throws(() => decode("clauses", count), /count exceeds/);
  assert.throws(() => decode("GameResourceReservationSetV1", Buffer.alloc(GAME_RESOURCE_MAX_SET_BYTES_V1 + 1)), /bounded/);
  const bytes = encode("GameResourceReservationClauseV1", clause(1));
  assert.ok(bytes[0] < 128);
  assert.throws(() => decode("GameResourceReservationClauseV1", Buffer.concat([Buffer.of(bytes[0] | 128, 0), bytes.subarray(1)])), /noncanonical/);
  const fixedLength = Buffer.alloc(8); fixedLength.writeBigUInt64LE(BigInt(bytes[0]));
  assert.throws(() => decode("GameResourceReservationClauseV1", Buffer.concat([fixedLength, bytes.subarray(1)])));
  assert.throws(() => decode("GameResourceReservationClauseV1", Buffer.of(255, 255, 255)), /length exceeds/);
  assert.throws(() => decode("GameResourceReservationClauseV1", [...bytes]), /Uint8Array/);
  const networkChanged = set(); networkChanged.network_id = hash(23);
  assert.notDeepEqual(encode("GameResourceReservationSetV1", set()), encode("GameResourceReservationSetV1", networkChanged));
});
