import { uriParser } from '../uri-app/node-uri-parser.js';

// Insure that tests all work even if the array prototype has been extended
// outside of the scope of the parser.
Array.prototype.foo = function () {
  console.log(this.length);
};

const p = uriParser;
let result = undefined;

describe('reproduce uri-js tests', () => {
  test('scheme', () => {
    result = p.parse('uri:');
    expect(result.uri).toBe('uri:');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBeUndefined();
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('userinfo', () => {
    result = p.parse('uri://@');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBe('');
    expect(result.host).toBe('');
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('empty host', () => {
    result = p.parse('uri://@:');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBe('');
    expect(result.host).toBe('');
    expect(result.port).toBe('');
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('host', () => {
    result = p.parse('uri://');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBe('');
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('port', () => {
    result = p.parse('uri://:');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBe('');
    expect(result.port).toBe('');
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('query', () => {
    result = p.parse('uri:?');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBeUndefined();
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('');
    expect(result.query).toBe('');
    expect(result.fragment).toBeUndefined();
  });
  test('fragment', () => {
    result = p.parse('uri:#');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBeUndefined();
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBe('');
  });
  test('all', () => {
    result = p.parse('uri://user:pass@example.com:123/one/two.three?q1=a1&q2=a2#body');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBe('user:pass');
    expect(result.host).toBe('example.com');
    expect(result.port).toBe(123);
    expect(result.path).toBe('/one/two.three');
    expect(result.query).toBe('q1=a1&q2=a2');
    expect(result.fragment).toBe('body');
  });
  test('IPv4address', () => {
    result = p.parse('uri://10.10.10.10');
    // console.dir(result);
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBe('10.10.10.10');
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('IPv6address', () => {
    result = p.parse('uri://[2001:db8::7]');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBe('2001:db8::7');
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('mixed IPv6address & IPv4address', () => {
    result = p.parse('uri://[::ffff:129.144.52.38]');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBe('::ffff:129.144.52.38');
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('mixed IPv4address & reg-name, example from terion-name (https://github.com/garycourt/uri-js/issues/4)', () => {
    result = p.parse('uri://10.10.10.10.example.com/en/process');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBe('10.10.10.10.example.com');
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('/en/process');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('IPv6address, example from bkw (https://github.com/garycourt/uri-js/pull/16)', () => {
    result = p.parse('uri://[2606:2800:220:1:248:1893:25c8:1946]/test');
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBe('2606:2800:220:1:248:1893:25c8:1946');
    expect(result.port).toBeUndefined();
    expect(result.path).toBe('/test');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
  test('IPv6address, example from RFC 5952)', () => {
    result = p.parse('uri://[2001:db8::1]:80');
    // console.dir(result);
    expect(result.scheme).toBe('uri');
    expect(result.userinfo).toBeUndefined();
    expect(result.host).toBe('2001:db8::1');
    expect(result.port).toBe(80);
    expect(result.path).toBe('');
    expect(result.query).toBeUndefined();
    expect(result.fragment).toBeUndefined();
  });
});
