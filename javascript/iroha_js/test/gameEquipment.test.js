import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { ed25519 } from '@noble/curves/ed25519';
import { AccountAddress } from '../src/address.js';
import { NetworkId } from '../src/networkId.js';
import { buildJoinGameSessionV1, encodeGameValueV1, decodeGameValueV1, gameHashLiteralV1, gameRosterHashV1,
  validateGameAdmissionBodyV1, validateGameResourceRecordsV1, validateGameResourceClausesV1, validateGameResourceRequirementsV1, decodeGameResourceValueV1 } from '../src/game.js';
import { encodeNftMarketValueV1, decodeNftMarketValueV1 } from '../src/nft.js';
import { buildBrowserInstructionTransactionPayload, browserTransactionPayloadHashHex } from '../src/transactionCodec.js';

const fixture = JSON.parse(readFileSync(new URL('fixtures/game-v1-codec.json', import.meta.url)));
const join = { ...fixture.vectors.find(row => row.name === 'JoinGameSessionV1').value, resources: [] };
const hash = n => gameHashLiteralV1(new Uint8Array(32).fill(n | 1));
const key = n => ed25519.getPublicKey(new Uint8Array(32).fill(n));
const account = n => AccountAddress.fromAccount({ algorithm: 'ed25519', publicKey: key(n) }).toI105();
const participant = n => ({ account: account(n), input_key: `ed0120${Buffer.from(key(n)).toString('hex').toUpperCase()}`, application_data: [n % 6] });
const policy = { kind: 'return_to_original_owner_at_terminal', value: null };
const clause = { nft_id: 'kit$equipment.universal', expected_metadata_hash: hash(3), role_id: hash(5), policy };
const body = () => ({ version: 1, participants: [participant(1), participant(2)], wagers: [{ slot: 0, nft_id: 'prize$equipment.universal', metadata_hash: hash(7) }], resources: [{ slot: 0, nft_id: clause.nft_id, metadata_hash: clause.expected_metadata_hash, role_id: clause.role_id, policy }] });

// Actual earlier native seven-field Join fixture, retained solely as a rejection vector.
const obsoleteJoin = Buffer.from('2001010101010101010101010101010101010101010101010101010101010101014A21000000000000000100018101390177010E01A8017D0117015F015601A30154016601C3014C017E01CC01CB018D018A019101B401EE013701A2015D01F6010F015B018F01C901B3019409010000000000000000010020E20F9BF2BACA090310128A293CF96D6F6CFCBF42F188C40C2DD926183EAC00CF20016801720145014E019C01040146014101AA0158011E01C501F30180011601190B0501000000010400000000', 'hex');

test('Join requires explicit resources, rejects the obsolete native layout and signs exact resource terms', () => {
  const required = { ...join }; delete required.resources;
  assert.throws(() => buildJoinGameSessionV1(required));
  assert.throws(() => decodeGameValueV1('JoinGameSessionV1', obsoleteJoin));
  assert.deepEqual(buildJoinGameSessionV1(join).JoinGameSessionV1.resources, []);
  const signed = { ...join, resources: [clause] };
  const payload = value => buildBrowserInstructionTransactionPayload({ networkId: NetworkId.parse(fixture.network_id), authority: account(1),
    instructions: [buildJoinGameSessionV1(value)], feePayment: { payer: 'authority', chargeLimits: [{ kind: 'nexus', assetDefinitionId: join.expected_asset_definition, maxAmount: '0.25' }] }, creationTimeMs: 1, ttlMs: 120000 });
  const digest = browserTransactionPayloadHashHex(payload(signed), 753);
  for (const changed of [{ ...clause, expected_metadata_hash: hash(9) }, { ...clause, role_id: hash(11) }, { ...clause, nft_id: 'other$equipment.universal' }]) {
    assert.notEqual(browserTransactionPayloadHashHex(payload({ ...signed, resources: [changed] }), 753), digest);
  }
  assert.throws(() => buildJoinGameSessionV1({ ...signed, resources: [{ ...clause, policy: { kind: 'return_to_winner_at_terminal', value: null } }] }));
});

test('one immutable admission body binds participants, wagers and returnable resources without defaults', () => {
  const original = body(), before = structuredClone(original);
  assert.deepEqual(validateGameAdmissionBodyV1(original), original);
  const digest = gameRosterHashV1(fixture.network_id, join.session_id, original);
  assert.notEqual(digest, gameRosterHashV1(fixture.network_id, join.session_id, { ...original, resources: [] }));
  assert.notEqual(digest, gameRosterHashV1(fixture.network_id, join.session_id, { ...original, wagers: [] }));
  assert.notEqual(digest, gameRosterHashV1(hash(13), join.session_id, original));
  assert.notEqual(digest, gameRosterHashV1(fixture.network_id, hash(15), original));
  for (const name of ['participants', 'wagers', 'resources', 'version']) { const missing = { ...original }; delete missing[name]; assert.throws(() => validateGameAdmissionBodyV1(missing)); }
  for (const name of ['GameRosterBodyV1', 'GameItemRosterBodyV1']) assert.throws(() => encodeGameValueV1(name, {}));
  assert.throws(() => gameRosterHashV1(fixture.network_id, join.session_id));
  assert.throws(() => validateGameAdmissionBodyV1({ ...original, participants: original.participants.map(p => ({ ...p, dnf_at_tick: null })) }));
  assert.throws(() => validateGameAdmissionBodyV1({ ...original, resources: original.resources.map(r => ({ ...r, released_at_height: null })) }));
  assert.deepEqual(original, before);
});

test('admission rejects duplicate identities, resource/wager overlap, absent slots and unauthorized ordering', () => {
  const original = body();
  const invalid = [
    { ...original, participants: [original.participants[0], original.participants[0]] },
    { ...original, participants: [{ ...original.participants[0], input_key: original.participants[0].input_key.toLowerCase() }] },
    { ...original, resources: [{ ...original.resources[0], nft_id: original.wagers[0].nft_id }] },
    { ...original, resources: [{ ...original.resources[0], slot: 2 }] },
    { ...original, wagers: [{ ...original.wagers[0], slot: 1 }, original.wagers[0]] },
    { ...original, resources: [original.resources[0], { ...original.resources[0], nft_id: 'second$equipment.universal' }] },
    { ...original, resources: Array.from({ length: 5 }, (_, i) => ({ ...original.resources[0], nft_id: `kit${i}$equipment.universal`, role_id: hash(i * 2 + 1) })) },
    { ...original, participants: [{ ...original.participants[0], application_data: Array(4097).fill(0) }] },
    { ...original, resources: [{ ...original.resources[0], nft_id: `${'x'.repeat(513)}$equipment.universal` }] },
  ];
  for (const value of invalid) assert.throws(() => validateGameAdmissionBodyV1(value));
  let invoked = false;
  const hidden = [...original.resources]; Object.defineProperty(hidden, '0', { enumerable: true, get() { invoked = true; return original.resources[0]; } });
  assert.throws(() => validateGameAdmissionBodyV1({ ...original, resources: hidden })); assert.equal(invoked, false);
  const data = [0]; Object.defineProperty(data, '0', { enumerable: true, get() { invoked = true; return 0; } });
  assert.throws(() => validateGameAdmissionBodyV1({ ...original, participants: [{ ...original.participants[0], application_data: data }] })); assert.equal(invoked, false);
  assert.deepEqual(validateGameAdmissionBodyV1({ version: 1, participants: [], wagers: [], resources: [] }), { version: 1, participants: [], wagers: [], resources: [] });
});

test('native custody purposes distinguish equipment from wagers with no old game alias', () => {
  for (const [tag, kind] of ['sale', 'game_wager', 'game_resource'].entries()) {
    const value = { kind, value: null }, bytes = encodeNftMarketValueV1('NftCustodyPurposeV1', value);
    assert.equal(Buffer.from(bytes).readUInt32LE(), tag);
    assert.deepEqual(decodeNftMarketValueV1('NftCustodyPurposeV1', bytes), value);
  }
  assert.throws(() => encodeNftMarketValueV1('NftCustodyPurposeV1', { kind: 'game', value: null }));
  assert.throws(() => decodeNftMarketValueV1('NftCustodyPurposeV1', Buffer.from([3, 0, 0, 0])));
});

test('retained equipment event records validate common terminal return heights and original owners', () => {
  const record = { slot: 0, nft_id: clause.nft_id, metadata_hash: clause.expected_metadata_hash, role_id: clause.role_id, policy, original_owner: account(1), custody: account(3), reserved_at_height: '10', released_at_height: '20' };
  assert.deepEqual(validateGameResourceRecordsV1([record], [account(1), account(2)]), [record]);
  assert.throws(() => validateGameResourceRecordsV1([record], [account(2), account(1)]));
  const event = { session_id: join.session_id, revision: '2', phase: 6, dispute_root: hash(17), payout_claims: [], item_stakes: [], resources: [record], terminal_at_height: '20' };
  assert.deepEqual(decodeGameValueV1('GameSessionEventV1', encodeGameValueV1('GameSessionEventV1', event)), event);
});


test('ambiguous dotted NFT components cannot enter equipment authorization or admission', () => {
  // Native typed components can share this short display while identifying
  // different NFTs. Browser authorization must never choose either silently.
  const components = [['art.gallery', 'universal'], ['art', 'gallery.universal']];
  assert.equal(...components.map(([domain, dataspace]) => `kit$${domain}.${dataspace}`));
  const original = body();
  for (const [domain, dataspace] of components) {
    const nft_id = `kit$${domain}.${dataspace}`;
    assert.equal(Buffer.byteLength(nft_id), 25, 'this is ambiguity, not an oversized identifier');
    const changed = { ...clause, nft_id };
    const retained = { slot: 0, nft_id, metadata_hash: clause.expected_metadata_hash, role_id: clause.role_id, policy, original_owner: account(1), custody: account(3), reserved_at_height: '10', released_at_height: null };
    for (const action of [
      () => encodeNftMarketValueV1('nft', nft_id),
      () => buildJoinGameSessionV1({ ...join, resources: [changed] }),
      () => validateGameResourceClausesV1([changed]),
      () => validateGameResourceRequirementsV1([changed]),
      () => validateGameResourceRecordsV1([retained], [account(1)]),
      () => validateGameAdmissionBodyV1({ ...original, wagers: [{ ...original.wagers[0], nft_id }] }),
      () => validateGameAdmissionBodyV1({ ...original, resources: [{ ...original.resources[0], nft_id }] }),
    ]) assert.throws(action, /exact domain\.dataspace/);
  }
  // Deliberately construct short adversarial bare fields, not native goldens.
  // The control proves the framing is otherwise accepted before changing each
  // native domain component. Canonical decode must reject both ambiguous forms.
  const field = bytes => { assert.ok(bytes.length < 128); return Buffer.concat([Buffer.of(bytes.length), bytes]); };
  const name = value => field(Buffer.from(value));
  const nft = (domain, dataspace) => Buffer.concat([field(Buffer.concat([field(name(domain)), field(name(dataspace))])), field(name('kit'))]);
  assert.deepEqual(nft('art', 'universal'), Buffer.from(encodeNftMarketValueV1('nft', 'kit$art.universal')));
  for (const [domain, dataspace] of components) {
    const bytes = nft(domain, dataspace);
    assert.throws(() => decodeNftMarketValueV1('nft', bytes), /exact domain\.dataspace/);
    const resource = Buffer.concat([field(bytes), field(Buffer.alloc(32, 3)), field(Buffer.alloc(32, 5)), field(Buffer.alloc(4))]);
    assert.throws(() => decodeGameResourceValueV1('GameResourceReservationClauseV1', resource), /exact domain\.dataspace/);
  }
});
