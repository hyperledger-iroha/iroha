import { default as Grammar } from './node-grammar.js';
import { Parser, utilities as utils, identifiers as id } from '../lib/parser.js';

const obj = new Grammar();
const parser = new Parser();
parser.callbacks['float'] = (sys, chars, phraseIndex, data) => {
  switch (sys.state) {
    case id.ACTIVE:
      data.float = '';
      data.sign = '';
      data.decimal = '';
      data.exponent = '';
      break;
    case id.MATCH:
      data.float = utils.charsToString(chars, phraseIndex, sys.phraseLength);
      break;
  }
};
parser.callbacks['sign'] = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.sign = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
parser.callbacks['decimal'] = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.decimal = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
parser.callbacks['exponent'] = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.exponent = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
const data = {};
const input = '+1.234e-10';
const result = parser.parse(obj, 'float', input, data);
console.log('\nPARSER ABNF GRAMMAR');
console.log(obj.toString());
console.log('\nPARSER INPUT STRING');
console.log(input);
console.log('\nPARSER RESULT');
console.dir(result);
if (result.success) {
  console.log('\nPARSED DATA');
  console.log(JSON.stringify(data, null, 2));
} else {
  console.log('\nnot a valid floating point number');
}
