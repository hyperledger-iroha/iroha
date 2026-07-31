import { default as Grammar } from './node-grammar.js';
import { Parser, identifiers as id, utilities as utils } from '../lib/parser.js';

// This is the required `UDT` callback function.
// Rule name callback functions are optional, however
// when `UDT`s are present in the grammar,
// the parser will fail with an error message if all of the `UDT`s in the grammar
// do not have callback functions defined for them.
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
      data['u_office'] = utils.charsToString(chars, phraseIndex, sys.phraseLength);
    }
  } else {
    sys.state = id.NOMATCH;
    sys.phraseLength = 0;
  }
};

// Define some rule name (`RNM`) callback functions.
const phoneNumber = (sys, chars, phraseIndex, data) => {
  switch (sys.state) {
    case id.ACTIVE:
      data['phone-number'] = 'invalid';
      data['area-code'] = undefined;
      data['u_office'] = undefined;
      data['subscriber'] = undefined;
      break;
    case id.MATCH:
      data['phone-number'] = utils.charsToString(chars, phraseIndex, sys.phraseLength);
      break;
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
    data['subscriber'] = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
const obj = new Grammar();
const parser = new Parser();
parser.callbacks['phone-number'] = phoneNumber;
parser.callbacks['area-code'] = areaCode;
parser.callbacks['u_office'] = UDToffice;
parser.callbacks['subscriber'] = subscriber;
let data = {};
const result = parser.parse(obj, 'phone-number', '(339)501-1234', data);
console.dir(result);
if (result.success) {
  console.dir(data);
} else {
  console.log(`phone-number: ${data['phone-number']}`);
}
