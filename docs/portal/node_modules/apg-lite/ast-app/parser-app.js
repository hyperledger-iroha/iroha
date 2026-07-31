import { default as Grammar } from './node-grammar.js';
import { Parser, utilities as utils, identifiers as id } from '../lib/parser.js';

// parser callback functions
const pAuthority = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.ACTIVE) {
    data.authority = '';
    data.userinfo = undefined;
    data.host = '';
    data.port = undefined;
  } else if (sys.state === id.MATCH) {
    data.authority = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
const pUserinfo = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.userinfo = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
const pHost = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.host = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
const pPort = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data.port = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};

// use the parser callback functions
const input = 'example.com:80';
const parser = new Parser();
parser.callbacks.authority = pAuthority;
parser.callbacks.userinfo = pUserinfo;
parser.callbacks.host = pHost;
parser.callbacks.port = pPort;
let data = {};
const grammarObject = new Grammar();
const result = parser.parse(grammarObject, 'authority', input, data);
console.log('\nTHE ABNF GRAMMAR DEFINING THE PARSER');
console.log(grammarObject.toString());
console.log('\nPARSER INPUT STRING');
console.log(input);
console.log('\nPARSER RESULT');
console.dir(result);
console.log('\nPARSER CALLBACK FUNCTIONS PHRASES');
console.dir(data);
let note = 'Note that the parse succeded but the captured phrases are wrong.\n';
note += 'A valid "authority" phrase was captured but "[authority "@"]" failed.\n';
note += 'With the parser callback functions, there was no mechanism to\n';
note += '"uncapture" the authority phrase.\n';
console.log('\nPARSER NOTE');
console.log(note);
