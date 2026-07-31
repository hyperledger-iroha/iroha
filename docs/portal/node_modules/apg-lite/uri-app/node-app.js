import { uriParser } from './node-uri-parser.js';
const args = process.argv.slice(2);

Array.prototype.foo = function () {
  console.log(this.length);
};
let uri;
if (args.length === 0) {
  uri = 'https://user:pass@example.com:123/one/two.three?q1=a1&q2=a2#body';
} else {
  uri = args[0];
}
console.log(`PARSE OF URI "${uri}"`);
const doTrace = false;
const result = uriParser.parse(uri, doTrace);
console.log('\nAPG PARSER DETAILS');
console.dir(uriParser.apgParserResult());
if (result) {
  console.log(`\nURI PARSER RESULT`);
  console.dir(result);
  // or
  // console.log(JSON.stringify(result, null, 2));
} else {
  console.log(`\nUNSUCCESSFUL URI PARSER RESULT - INVALID URI`);
}
if (doTrace) {
  console.log('\nTRACE');
  console.log(uriParser.displayTrace());
}
