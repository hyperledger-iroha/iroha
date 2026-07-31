// import { cwd } from 'node:process';
// console.log(`cwd: ${cwd()}`);
import { Parser, identifiers as id, utilities as utils } from '../lib/parser.js';

// Insure that tests all work even if the array prototype has been extended
// outside of the scope of the parser.
Array.prototype.foo = function () {
  console.log(this.length);
};

const p = new Parser();
let result;
let str;
describe('test display of identifier names', () => {
  test('opcode identifiers', () => {
    expect(id.idName(id.ALT)).toBe('ALT');
    expect(id.idName(id.CAT)).toBe('CAT');
    expect(id.idName(id.REP)).toBe('REP');
    expect(id.idName(id.RNM)).toBe('RNM');
    expect(id.idName(id.TRG)).toBe('TRG');
    expect(id.idName(id.TBS)).toBe('TBS');
    expect(id.idName(id.TLS)).toBe('TLS');
    expect(id.idName(id.UDT)).toBe('UDT');
    expect(id.idName(id.AND)).toBe('AND');
    expect(id.idName(id.NOT)).toBe('NOT');
    expect(id.idName(id.BKR)).toBe('UNRECOGNIZED STATE');
  });
  test('parser state identifiers', () => {
    expect(id.idName(id.ACTIVE)).toBe('ACTIVE');
    expect(id.idName(id.EMPTY)).toBe('EMPTY');
    expect(id.idName(id.MATCH)).toBe('MATCH');
    expect(id.idName(id.NOMATCH)).toBe('NOMATCH');
    expect(id.idName(999)).toBe('UNRECOGNIZED STATE');
  });
  test('ast identifiers', () => {
    expect(id.idName(id.SEM_PRE)).toBe('SEM_PRE');
    expect(id.idName(id.SEM_POST)).toBe('SEM_POST');
    expect(id.idName(id.SEM_OK)).toBe('SEM_OK');
    expect(id.idName(id.SEM_SKIP)).toBe('UNRECOGNIZED STATE');
  });
});
describe('test conversion of JavaScript string to array of UTF-16 character codes', () => {
  test('abc', () => {
    str = 'abc';
    result = utils.stringToChars(str);
    expect(result[0]).toBe(97);
    expect(result[1]).toBe(98);
    expect(result[2]).toBe(99);
    expect(result).toEqual([97, 98, 99]);
  });
  test('cool shades emoticon', () => {
    str = 'a \uD83D\uDE0E b';
    result = utils.stringToChars(str);
    expect(result).toEqual([97, 32, 128526, 32, 98]);
  });
});
describe('test conversion of array of UTF-16 character codes to JavaScript string', () => {
  const a = [97, 32, 128526, 32, 98];
  test('whole string', () => {
    str = utils.charsToString(a);
    expect(str).toEqual('a \uD83D\uDE0E b');
  });
  test('first 3', () => {
    str = utils.charsToString(a, 0, 3);
    expect(str).toEqual('a \uD83D\uDE0E');
  });
  test('last 3', () => {
    str = utils.charsToString(a, 2);
    expect(str).toEqual('\uD83D\uDE0E b');
  });
  test('middle', () => {
    str = utils.charsToString(a, 2, 1);
    expect(str).toEqual('\uD83D\uDE0E');
  });
  test('substrings - 1', () => {
    str = '0123456789';
    result = utils.stringToChars(str);
    str = utils.charsToString(result, 2);
    expect(str).toEqual('23456789');
  });
  test('substrings - 2', () => {
    str = '0123456789';
    result = utils.stringToChars(str);
    str = utils.charsToString(result, 2, 100);
    expect(str).toEqual('23456789');
  });
  test('substrings - 3', () => {
    str = '0123456789';
    result = utils.stringToChars(str);
    str = utils.charsToString(result, 2, 3);
    expect(str).toEqual('234');
  });
  test('substrings - 4', () => {
    str = '0123456789';
    result = utils.stringToChars(str);
    str = utils.charsToString(result, 2, 0);
    expect(str).toEqual('');
  });
  test('substrings - 5', () => {
    str = '0123456789';
    result = utils.stringToChars(str);
    str = utils.charsToString(result, 2, -1);
    expect(str).toEqual('');
  });
});
