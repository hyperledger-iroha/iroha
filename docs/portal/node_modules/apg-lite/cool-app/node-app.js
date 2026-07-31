import { default as Grammar } from './node-grammar.js';
import { Parser, Trace, utilities as utils, identifiers as id } from '../lib/parser.js';

const obj = new Grammar();
const parser = new Parser();
parser.callbacks['start'] = (sys, chars, phraseIndex, data) => {
  switch (sys.state) {
    case id.ACTIVE:
      data.start = 'invalid statement(s)';
      data.statement1 = undefined;
      data.statement2 = undefined;
      break;
    case id.MATCH:
      data.start = utils.charsToString(chars, phraseIndex, sys.phraseLength);
      break;
  }
};
parser.callbacks['statement1'] = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.statement1 = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
parser.callbacks['statement2'] = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.statement2 = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
const data = {};
let input = 'See you later alligator. \uD83D\uDE01\n';
input += 'After while crocodile. \uD83D\uDE0E\n';
parser.trace = new Trace();
const result = parser.parse(obj, 'start', input, data);
console.log('\nPARSER INPUT STRING');
console.log(input);
console.log('\nPARSER RESULT');
console.dir(result);
if (result.success) {
  console.log('\nPARSED DATA');
  console.dir(data);
} else {
  console.log('\nOne or more invalid statements.');
  console.log(parser.trace.displayTrace());
}
