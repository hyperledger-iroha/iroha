import { default as Grammar } from './negative-grammar.js';
import { Parser, utilities as utils, identifiers as id } from '../lib/parser.js';

// parser callback functions
const cLang = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.ACTIVE) {
    data.comment = '';
  }
};
const comment = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.comment = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};

// use the parser callback functions
const input = '/****** /******/';
const parser = new Parser();
parser.callbacks['C-lang'] = cLang;
parser.callbacks.comment = comment;
let data = {};
const grammarObject = new Grammar();
const result = parser.parse(grammarObject, 'c-lang', input, data);
console.log('\nTHE NEGATIVE LOOKAHEAD APPLICATION\n');
console.log('THE SABNF GRAMMAR DEFINING THE PARSER');
console.log(grammarObject.toString());
console.log('PARSER INPUT STRING');
console.log(input);
console.log('\nPARSER RESULT');
console.dir(result);
console.log('\nPARSER CALLBACK FUNCTIONS PHRASES');
console.dir(data);
let note = 'Note that the parser accepts "*" and "/" as comment characters but not "*/".\n';
console.log('\nPARSER NOTE');
console.log(note);
