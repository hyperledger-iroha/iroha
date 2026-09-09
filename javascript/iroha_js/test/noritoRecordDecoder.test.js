import assert from 'node:assert/strict';
import test from 'node:test';
import { createNoritoRecordDecoder, createNoritoRecordEncoder } from '../src/noritoRecordDecoder.js';

test('record decoding keeps field and callback order, optional context and indexed vector paths', () => {
  const calls = [];
  const payload = { id: 3, optional: 7, entries: [11, 13] };
  const struct = (value, context, keys) => {
    assert.equal(value, payload);
    assert.equal(context, 'record');
    assert.deepEqual(keys, ['id', 'optional', 'entries']);
    calls.push('frame');
    return value;
  };
  const scalar = (value, context) => { calls.push([context, value]); return value; };
  const optional = (value, callback, context) => { calls.push('option'); return callback(value, context); };
  const vector = (value, callback, context) => { assert.equal(context, 'record.entries'); calls.push('vector'); return value.map(callback); };
  const decode = createNoritoRecordDecoder(struct, optional, vector);
  const result = decode(payload, 'record', [['id', scalar, 0], ['optional', scalar, 1], ['entries', scalar, 2]]);
  assert.deepEqual(result, payload);
  assert.equal(Object.getPrototypeOf(result), Object.prototype);
  assert.deepEqual(Object.keys(result), ['id', 'optional', 'entries']);
  assert.deepEqual(calls, ['frame', ['record.id', 3], 'option', ['record.optional', 7], 'vector', ['record.entries[0]', 11], ['record.entries[1]', 13]]);
});

test('a malformed record frame prevents every field decode and preserves its original failure', () => {
  const failure = new RangeError('truncated record');
  const never = () => assert.fail('field processing must not precede frame admission');
  const decode = createNoritoRecordDecoder(() => { throw failure; }, never, never);
  assert.throws(() => decode(new Uint8Array(), 'record', [['id', never, 0]]), error => error === failure);
});

test('an invalid optional field prevents later fields and preserves the primitive failure', () => {
  const failure = new TypeError('record.optional invalid option tag');
  const never = () => assert.fail('later field must not run');
  const decode = createNoritoRecordDecoder(value => value, (_value, _decode, context) => {
    assert.equal(context, 'record.optional'); throw failure;
  }, never);
  assert.throws(() => decode({optional: 2, later: 3}, 'record', [['optional', never, 1], ['later', never, 0]]), error => error === failure);
});

test('vector elements retain their index and terminate at the original failing decoder', () => {
  const calls = [], failure = new TypeError('invalid item');
  const scalar = (value, context) => { calls.push(context); if (value === 2) throw failure; return value; };
  const decode = createNoritoRecordDecoder(value => value, () => assert.fail('not optional'), (value, callback) => value.map(callback));
  assert.throws(() => decode({items: [1, 2, 3]}, 'record', [['items', scalar, 2]]), error => error === failure);
  assert.deepEqual(calls, ['record.items[0]', 'record.items[1]']);
});

test('record encoding reads the parent for every field and frames after ordered primitive calls', () => {
  const calls = [];
  let parents = 0;
  const record = {
    get body() {
      calls.push(['parent', ++parents]);
      return { id: parents * 10, optional: parents * 10, items: [parents * 10, parents * 10 + 1] };
    },
  };
  const scalar = (value, context) => { calls.push([context, value]); return value; };
  const encode = createNoritoRecordEncoder(fields => { calls.push('frame'); return fields; },
    (value, callback, context) => { calls.push('option'); return callback(value, context); },
    (value, callback) => { calls.push('vector'); return value.map(callback); });
  assert.deepEqual(encode(record, 'record', [
    ['id', null, 0, scalar, 0], ['optional', null, 0, scalar, 1], ['items', null, 0, scalar, 2],
  ], 'body'), [[10], [20], [[30, 31]]]);
  assert.deepEqual(calls, [
    ['parent', 1], ['record.id', 10], ['parent', 2], 'option', ['record.optional', 20],
    ['parent', 3], 'vector', ['record.items[0]', 30], ['record.items[1]', 31], 'frame',
  ]);
});

test('encoding preserves a getter failure before later getters or framing', () => {
  const failure = new RangeError('record.first getter failure');
  const never = () => assert.fail('later encoding work must not run');
  const encode = createNoritoRecordEncoder(never, never, never);
  const record = { get first() { throw failure; }, get later() { return never(); } };
  assert.throws(() => encode(record, 'record', [
    ['first', null, 0, never, 0], ['later', null, 0, never, 0],
  ]), error => error === failure);
});

test('empty and invalid encoded vectors do not coerce the element context', () => {
  const failure = new TypeError('invalid vector input');
  const never = () => assert.fail('an absent element must not coerce or encode');
  const context = { toString: never };
  const encode = createNoritoRecordEncoder(fields => fields, never, (value, callback) => {
    if (!Array.isArray(value)) throw failure;
    return value.map(callback);
  });
  for (const kind of [2, 3]) {
    const schema = [['items', null, 0, never, kind]];
    assert.deepEqual(encode({ items: [] }, context, schema), [[[]]]);
    assert.throws(() => encode({ items: {} }, context, schema), error => error === failure);
  }
  for (const value of [undefined, null]) {
    assert.deepEqual(encode({ items: value }, context, [['items', null, 0, never, 3]]), [[[]]]);
    assert.throws(() => encode({ items: value }, context, [['items', null, 0, never, 2]]),
      error => error === failure);
  }
});

test('encoded vectors coerce each element context after vector admission', () => {
  const calls = [];
  let coercions = 0;
  const context = { toString() { calls.push('coerce'); return `record${++coercions}`; } };
  const scalar = (value, path) => { calls.push([path, value]); return value; };
  const encode = createNoritoRecordEncoder(fields => fields, () => assert.fail('not optional'),
    (value, callback) => { calls.push('vector'); return value.map(callback); });
  assert.deepEqual(encode({ items: [3, 5] }, context, [['items', null, 0, scalar, 2]]), [[[3, 5]]]);
  assert.deepEqual(calls, ['vector', 'coerce', ['record1.items[0]', 3], 'coerce', ['record2.items[1]', 5]]);
});

test('an encoded vector element failure prevents later coercions, fields and framing', () => {
  const failure = new TypeError('second encoded element invalid');
  const calls = [];
  const never = () => assert.fail('later encoding work must not run');
  const context = { toString() { calls.push('coerce'); return 'record'; } };
  const scalar = (value, path) => { calls.push(path); if (value === 2) throw failure; return value; };
  const encode = createNoritoRecordEncoder(never, never, (value, callback) => value.map(callback));
  const record = { items: [1, 2, 3], get later() { return never(); } };
  assert.throws(() => encode(record, context, [
    ['items', null, 0, scalar, 2], ['later', null, 0, never, 0],
  ]), error => error === failure);
  assert.deepEqual(calls, ['coerce', 'record.items[0]', 'coerce', 'record.items[1]']);
});

test('encoded options retain null and undefined and evaluate their context once', () => {
  const never = () => assert.fail('absent optional value must not encode');
  for (const value of [null, undefined]) {
    let coercions = 0;
    const context = { toString() { coercions += 1; return 'record'; } };
    const encode = createNoritoRecordEncoder(fields => fields, (actual, callback, path) => {
      assert.equal(actual, value);
      assert.equal(callback, never);
      assert.equal(path, 'record.optional');
      return 'absent';
    }, never);
    assert.deepEqual(encode({ optional: value }, context, [['optional', null, 0, never, 1]]), [['absent']]);
    assert.equal(coercions, 1);
  }
});
