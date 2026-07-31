import * as fs from 'node:fs';
// import { cwd } from 'node:process';
// console.log(`Current Working Directory: ${cwd()}`);
import { default as Grammar } from '../uri-app/node-grammar.js';
import { Parser, Trace } from '../lib/parser.js';

// Insure that tests all work even if the array prototype has been extended
// outside of the scope of the parser.
Array.prototype.foo = function () {
  console.log(this.length);
};

const grammarObj = new Grammar();
const alpha = 'abcdefghijklmnopqrstuvwxyz';
const ALPHA = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
const digit = '0123456789';
const unreservedS = '-._~';
const unreserved = alpha + ALPHA + digit + unreservedS;
const genDelims = ':/?#[]@';
const subDelims = "!$&'()*+,;=";
const reserved = genDelims + subDelims;
const pctEncoded = '%00%ff%FF%0a%0A%20';
const pchar = unreserved + subDelims + ':@' + pctEncoded;
const scheme = alpha + ALPHA + digit + '+-.';
const userinfo = unreserved + subDelims + ':' + pctEncoded;
const IPvFuture = 'v123.' + unreserved + subDelims + ':';
const regName = unreserved + subDelims + pctEncoded;
const fragment = pchar + '/?';
const dir = './output';
const doParse = function doParse(rule, input, doTrace) {
  const parser = new Parser();
  if (doTrace) {
    parser.trace = new Trace();
  }
  const result = parser.parse(grammarObj, rule, input);
  if (doTrace) {
    const html = parser.trace.displayTrace();
    const name = `${dir}/apg-lite-uri-parser-${rule}.txt`;
    try {
      fs.mkdirSync(dir);
    } catch (e) {
      if (e.code !== 'EEXIST') {
        throw new Error(`fs.mkdir failed: ${e.message}`);
      }
    }
    fs.writeFileSync(name, html);
    console.log(`view "${name}" to display parser's trace`);
    console.dir(result);
  }
  return result;
};
describe('test strings with explicit special character definitions', () => {
  test('test pchar', () => {
    const rule = 'segment-nz';
    let result = doParse(rule, pchar, false);
    expect(result.success).toBe(true);
    result = doParse(rule, pchar + '/', false);
    expect(result.success).toBe(false);
    result = doParse(rule, pchar + '?', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '/', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '?', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '#', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '[', false);
    expect(result.success).toBe(false);
  });
  test('test scheme', () => {
    const rule = 'scheme';
    let result = doParse(rule, scheme, false);
    expect(result.success).toBe(true);
    result = doParse(rule, ':', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '#', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '?', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '@', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '!', false);
    expect(result.success).toBe(false);
  });
  test('test userinfo', () => {
    const rule = 'userinfo';
    let result = doParse(rule, userinfo, false);
    expect(result.success).toBe(true);
    result = doParse(rule, '/', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '#', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '?', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '@', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '[', false);
    expect(result.success).toBe(false);
    result = doParse(rule, ']', false);
    expect(result.success).toBe(false);
  });
  test('test IPvFuture', () => {
    const rule = 'IPvFuture';
    let result = doParse(rule, IPvFuture, false);
    expect(result.success).toBe(true);
    result = doParse(rule, '/', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '#', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '?', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '@', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '[', false);
    expect(result.success).toBe(false);
    result = doParse(rule, ']', false);
    expect(result.success).toBe(false);
  });
  test('test reg-name', () => {
    const rule = 'reg-name';
    let result = doParse(rule, regName, false);
    expect(result.success).toBe(true);
    result = doParse(rule, '/', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '#', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '?', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '@', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '[', false);
    expect(result.success).toBe(false);
    result = doParse(rule, ']', false);
    expect(result.success).toBe(false);
  });
  test('test fragment', () => {
    const rule = 'fragment';
    let result = doParse(rule, fragment, false);
    expect(result.success).toBe(true);
    result = doParse(rule, '#', false);
    expect(result.success).toBe(false);
    result = doParse(rule, '[', false);
    expect(result.success).toBe(false);
  });
});
