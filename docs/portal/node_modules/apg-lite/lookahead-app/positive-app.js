import { default as Grammar } from './positive-grammar.js';
import { Parser } from '../lib/parser.js';

const parser = new Parser();
const grammarObject = new Grammar();
let result;
let success;
let input;
let comment;

console.log('THE POSITIVE LOOKAHEAD APPLICATION\n');
console.log('THE SABNF GRAMMAR DEFINING THE PARSER');
console.log(grammarObject.toString());

// correct
comment = 'equal numbers of a, b, c - should be ok';
input = 'aaabbbccc';
result = parser.parse(grammarObject, 'S', input);
console.log('\nINPUT STRING: RESULT (COMMENT)');
success = result.success ? 'success' : 'failure';
console.log(`${input}: ${success} (${comment})`);

// too many a's
comment = "too many a's";
input = 'aaaabbbccc';
result = parser.parse(grammarObject, 'S', input);
success = result.success ? 'success' : 'failure';
console.log(`${input}: ${success} (${comment})`);

// too few a's
comment = "too few a's";
input = 'aabbbccc';
result = parser.parse(grammarObject, 'S', input);
success = result.success ? 'success' : 'failure';
console.log(`${input}: ${success} (${comment})`);

// too many b's
comment = "too many b's";
input = 'aaabbbbccc';
result = parser.parse(grammarObject, 'S', input);
success = result.success ? 'success' : 'failure';
console.log(`${input}: ${success} (${comment})`);

// too few b's
comment = "too few b's";
input = 'aaabbccc';
result = parser.parse(grammarObject, 'S', input);
success = result.success ? 'success' : 'failure';
console.log(`${input}: ${success} (${comment})`);

// too many c's
comment = "too many c's";
input = 'aaabbbcccc';
result = parser.parse(grammarObject, 'S', input);
success = result.success ? 'success' : 'failure';
console.log(`${input}: ${success} (${comment})`);

// too few c's
comment = "too few c's";
input = 'aaabbbcc';
result = parser.parse(grammarObject, 'S', input);
success = result.success ? 'success' : 'failure';
console.log(`${input}: ${success} (${comment})`);
