// import { cwd } from 'node:process';
// console.log(`cwd: ${cwd()}`);
import { Parser, Trace, identifiers as id, utilities as utils } from '../lib/parser.js';
import { default as Grammar } from './phone-grammar.js';

// Insure that tests all work even if the array prototype has been extended
// outside of the scope of the parser.
Array.prototype.foo = function () {
  console.log(this.length);
};

// define the RNM and UDT callback functions
const UDToffice = (sys, chars, phraseIndex, data) => {
  let matchFound = false;
  while (1) {
    if (chars + phraseIndex + 3 <= chars.length) {
      /* not three digits left in the string */
      break;
    }
    const dig1 = chars[phraseIndex];
    const dig2 = chars[phraseIndex + 1];
    const dig3 = chars[phraseIndex + 2];
    if (dig1 < 50 || dig1 > 57) {
      /* first digit must be in range 2-9 */
      break;
    }
    if (dig2 < 48 || dig2 > 57 || dig3 < 48 || dig3 > 57) {
      /* second & third digits must be in range 0-9 */
      break;
    }
    if (dig2 === 49 && dig3 === 49) {
      /* if the second digit is "1" then the third digit cannot also be "1" */
      break;
    }
    matchFound = true;
    break;
  }
  if (matchFound === true) {
    sys.state = id.MATCH;
    sys.phraseLength = 3;
    if (data !== null) {
      data.u_office = utils.charsToString(chars, phraseIndex, sys.phraseLength);
    }
  } else {
    sys.state = id.NOMATCH;
    sys.phraseLength = 0;
  }
};

// Define some rule name (`RNM`) callback functions.
const phoneNumber = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    /* initialize the data array length to zero */
    data.length = 0;
  }
};
const areaCode = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    /* capture the area code */
    data['area-code'] = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
const subscriber = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    /* capture the 4-digit subscriber number */
    data.subscriber = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};

// set up the parser
const obj = new Grammar();
const parser = new Parser();
parser.trace = new Trace();
parser.callbacks['phone-number'] = phoneNumber;
parser.callbacks['area-code'] = areaCode;
parser.callbacks.u_office = UDToffice;
parser.callbacks.subscriber = subscriber;
let data = [];

test('test the trace display for rules and UDTs', () => {
  const result = parser.parse(obj, 'phone-number', '(339)501-1234', data);
  expect(result.success).toBe(true);
  expect(data['area-code']).toBe('339');
  expect(data['u_office']).toBe('501');
  expect(data['subscriber']).toBe('1234');
  const str = parser.trace.displayTrace();
  expect(/\|M\|\[RNM\(phone\-number\)\]/.test(str)).toBe(true);
  expect(/\|M\|\[RNM\(area\-code\)\]/.test(str)).toBe(true);
  expect(/\|M\|\[UDT\(u_office\)\]/.test(str)).toBe(true);
  expect(/\|M\|\[RNM\(subscriber\)\]/.test(str)).toBe(true);
});
