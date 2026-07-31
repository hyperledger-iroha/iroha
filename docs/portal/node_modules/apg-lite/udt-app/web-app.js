const parser = new Parser();
const grammar = new Grammar();
const id = identifiers;
const utils = utilities;

// display the ABNF grammar on the web page
$('#grammar-bnf').html(grammar.toString());

// define the parser's callback functions
parser.callbacks['phone-number'] = (sys, chars, phraseIndex, data) => {
  switch (sys.state) {
    case id.ACTIVE:
      data['phone-number'] = 'invalid';
      data.sign = undefined;
      data.decimal = undefined;
      data.exponent = undefined;
      break;
    case id.MATCH:
      data['phone-number'] = utils.charsToString(chars, phraseIndex, sys.phraseLength);
      break;
  }
};
parser.callbacks['u_office'] = (sys, chars, phraseIndex, data) => {
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
parser.callbacks['area-code'] = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data['area-code'] = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
parser.callbacks['subscriber'] = (sys, chars, phraseIndex, data) => {
  if (sys.state === id.MATCH) {
    data['subscriber'] = utils.charsToString(chars, phraseIndex, sys.phraseLength);
  }
};
const data = {};
const parseNumber = () => {
  // get the input string (the number to parse) from the web page
  const inputString = $('#string').val();

  // parse the input string
  const result = parser.parse(grammar, 'phone-number', inputString, data);

  // display the parser's result on the web page
  $('#result').html('Parser Result<br>' + JSON.stringify(result));
  let values = '';
  if (result.success) {
    // display the parsed values on the web page
    values += `Parsed Phone Number Parts\n`;
    values += `     pnone number: ${data['phone-number']}\n`;
    values += `        area code: ${data['area-code']}\n`;
    values += `      office code: ${data['u_office']}\n`;
    values += `subscriber number: ${data['subscriber']}\n`;
  } else {
    values += `Invalid Phone Number\n`;
  }
  $('#values').html(values);
};
$(document).ready(() => {
  $('#parse').click(parseNumber);
  $('#string').val('(339)501-5555');
});
