import { uriParser } from '../uri-app/node-uri-parser.js';

// Insure that tests all work even if the array prototype has been extended
// outside of the scope of the parser.
Array.prototype.foo = function () {
  console.log(this.length);
};

const p = uriParser;
let result = undefined;
let uri = '';

describe('test IPv4 addresses', () => {
  // NOTE: The reason for using the IP-literal form for host to test the IPv4address
  //       is that attempting to test the IPv4address form directly does not correctly
  //       identify malformed IPv4address. If it fails, for example with 1.1.1.256, then
  //       the host rule will simply move on to the reg-name alternative and it will succeed.
  //       That is, host = 1.1.1.256 is valid, but because it is a reg-name not an IPv4address.
  test('bad octets', () => {
    uri = 'uri://[::0.0.0.256]/p/path';
    result = p.parse(uri);
    expect(result).toBe(undefined);
    uri = 'uri://[::300.0.0.0]/p/path';
    result = p.parse(uri);
    expect(result).toBe(undefined);
    uri = 'uri://[::0.ff.0.255]/p/path';
    result = p.parse(uri);
    expect(result).toBe(undefined);
    uri = 'uri://[::0.0.256.0]/p/path';
    result = p.parse(uri);
    expect(result).toBe(undefined);
  });
  test('IPv4address 1', () => {
    uri = 'uri://[::10.10.10.10]';
    result = p.parse(uri);
    expect(result.host).toBe('::10.10.10.10');
  });
  test('IPv4address 2', () => {
    uri = 'uri://[::000.000.010.001]';
    result = p.parse(uri);
    expect(result.host).toBe('::000.000.010.001');
  });
  test('IPv4address 3', () => {
    uri = 'uri://[::001.099.200.255]';
    result = p.parse(uri);
    expect(result.host).toBe('::001.099.200.255');
  });
  test('IPv4address 4', () => {
    // version 1.0.4 and earlier would accept empty octets
    uri = 'uri://[::1..200.255]';
    result = p.parse(uri);
    expect(result).toBeUndefined();
  });
});
describe('test IPv6 addresses', () => {
  test('IPv6address no double colon', () => {
    uri = 'uri://[ffff:abcd:0:10:200:3000:f8a:1]';
    result = p.parse(uri);
    expect(result.host).toBe('ffff:abcd:0:10:200:3000:f8a:1');
    uri = 'uri://[ffff:abcd:0:10:200:3000:255.255.255.255]';
    result = p.parse(uri);
    expect(result.host).toBe('ffff:abcd:0:10:200:3000:255.255.255.255');
    // too many 16-bit digits
    uri = 'uri://[ffff:abcd:0:10:200:3000:f8a:1:ffff]';
    result = p.parse(uri);
    expect(result).toBe(undefined);
    // too few 16-bit digits
    result = p.parse('uri://[ffff:abcd:0:10:200:3000:f8a]');
    expect(result).toBe(undefined);
    // too many 16-bit digits
    result = p.parse('uri://[ffff:abcd:0:10:200:3000:ff:255.255.255.255]');
    expect(result).toBe(undefined);
    // too few 16-bit digits
    result = p.parse('uri://[ffff:abcd:0:10:200:255.255.255.255]');
    expect(result).toBe(undefined);
  });
  test('IPv6address leading double colon WITHOUT IPv4', () => {
    result = p.parse('uri://[::]');
    expect(result.host).toBe('::');
    result = p.parse('uri://[::ffff]');
    expect(result.host).toBe('::ffff');
    result = p.parse('uri://[::1:2:3:4:5:6:7]');
    expect(result.host).toBe('::1:2:3:4:5:6:7');
    // too many 16-bit digits
    result = p.parse('uri://[::1:2:3:4:5:6:7:8]');
    expect(result).toBe(undefined);
  });
  test('IPv6address leading double colon WITH IPv4', () => {
    result = p.parse('uri://[::198.162.10.255]');
    expect(result.host).toBe('::198.162.10.255');
    result = p.parse('uri://[::ffff:198.162.10.255]');
    expect(result.host).toBe('::ffff:198.162.10.255');
    result = p.parse('uri://[::1:2:3:4:5:198.162.10.255]');
    expect(result.host).toBe('::1:2:3:4:5:198.162.10.255');
    // too many 16-bit digits
    result = p.parse('uri://[::1:2:3:4:5:6:198.162.10.255]');
    expect(result).toBe(undefined);
  });
  test('IPv6address trailing double colon WITHOUT IPv4', () => {
    result = p.parse('uri://[1::]');
    expect(result.host).toBe('1::');
    result = p.parse('uri://[1:2:3:4:5:6:7::]');
    expect(result.host).toBe('1:2:3:4:5:6:7::');
    // too many 16-bit digits
    result = p.parse('uri://[1:2:3:4:5:6:7:8::]');
    expect(result).toBe(undefined);
  });
  test('IPv6address trailing double colon WITH IPv4', () => {
    result = p.parse('uri://[1::198.162.10.255]');
    expect(result.host).toBe('1::198.162.10.255');
    result = p.parse('uri://[1:2:3:4:5::198.162.10.255]');
    expect(result.host).toBe('1:2:3:4:5::198.162.10.255');
    // too many 16-bit digits
    result = p.parse('uri://[1:2:3:4:5:6::198.162.10.255]');
    expect(result).toBe(undefined);
  });
  test('IPv6address leading & trailing double colon WITHOUT IPv4', () => {
    result = p.parse('uri://[1::2]');
    expect(result.host).toBe('1::2');
    result = p.parse('uri://[1:2:3:4:5:6::7]');
    expect(result.host).toBe('1:2:3:4:5:6::7');
    result = p.parse('uri://[ffff:aaaa:bbbb::cccc:dddd:eeee:9999]');
    expect(result.host).toBe('ffff:aaaa:bbbb::cccc:dddd:eeee:9999');
    // too many 16-bit digits
    result = p.parse('uri://[1:2:3:4:5::6:7:8]');
    expect(result).toBe(undefined);
  });
  test('IPv6address leading & trailing double colon WITH IPv4', () => {
    result = p.parse('uri://[1::2:198.162.10.255]');
    expect(result.host).toBe('1::2:198.162.10.255');
    result = p.parse('uri://[1:2:3:4::7:198.162.10.255]');
    expect(result.host).toBe('1:2:3:4::7:198.162.10.255');
    result = p.parse('uri://[ffff:aaaa:bbbb::cccc:dddd:198.162.10.255]');
    expect(result.host).toBe('ffff:aaaa:bbbb::cccc:dddd:198.162.10.255');
    // too many 16-bit digits
    result = p.parse('uri://[1:2:3:4:5::6:198.162.10.255]');
    expect(result).toBe(undefined);
  });
});
