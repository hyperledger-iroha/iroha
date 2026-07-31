import { default as Grammar } from './node-grammar.js';
import { Parser, Ast, utilities as utils, identifiers as id } from '../lib/parser.js';

// AST callback functions
const astAuthority = (state, chars, phraseIndex, phraseLength, data) => {
  if (state === id.SEM_PRE) {
    data.authority = '';
    data.userinfo = undefined;
    data.host = '';
    data.port = undefined;
  } else if (state === id.SEM_POST) {
    let a = '';
    if (data.userinfo) {
      a = data.userinfo + '@';
    }
    a += data.host;
    if (data.port) {
      a += ':' + data.port;
    }
    data.authority = a;
  }
};
const astUserinfo = (state, chars, phraseIndex, phraseLength, data) => {
  if (state === id.SEM_PRE) {
    data.userinfo = utils.charsToString(chars, phraseIndex, phraseLength);
  }
};
const astHost = (state, chars, phraseIndex, phraseLength, data) => {
  if (state === id.SEM_PRE) {
    data.host = utils.charsToString(chars, phraseIndex, phraseLength);
  }
};
const astPort = (state, chars, phraseIndex, phraseLength, data) => {
  if (state === id.SEM_PRE) {
    data.port = utils.charsToString(chars, phraseIndex, phraseLength);
  }
};

// use the AST callback functions
const input = 'example.com:80';
const parser = new Parser();
parser.ast = new Ast();
parser.ast.callbacks.authority = astAuthority;
parser.ast.callbacks.userinfo = astUserinfo;
parser.ast.callbacks.host = astHost;
parser.ast.callbacks.port = astPort;
const grammarObject = new Grammar();
const result = parser.parse(grammarObject, 'authority', input);
console.log('\nTHE ABNF GRAMMAR DEFINING THE PARSER');
console.log(grammarObject.toString());
console.log('\nPARSER INPUT STRING');
console.log(input);
console.log('\nPARSER RESULT');
console.dir(result);
if (result.success) {
  let data = {};
  parser.ast.translate(data);
  console.log('\nAST CALLBACK FUNCTIONS PHRASES');
  console.dir(data);
  let note = 'Note that, unlike the parser callback functions, the AST callback functions\n';
  note += 'retain only the phases that were captured on ultimately successful branches of the parse tree.\n';
  console.log('\nAST NOTE');
  console.log(note);
  console.log('\nTHE AST IN XML FORMAT');
  console.log(parser.ast.toXml());
} else {
  console.log('\nPARSER FAILED');
}
