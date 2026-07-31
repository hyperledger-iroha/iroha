const parser = new Parser();
const grammar = new Grammar();
const id = identifiers;
const utils = utilities;

// display the ABNF grammar on the web page
$('#grammar-bnf').html(grammar.toString());

// define the parser's callback functions
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
const parseNumber = () => {
  // get the input string (the number to parse) from the web page
  const inputString = $('#string').val();

  // parse the input string
  const result = parser.parse(grammar, 'float', inputString, data);

  // display the parser's result on the web page
  $('#result').html('Parser Result<br>' + JSON.stringify(result));
  let values = '';
  if (result.success) {
    // display the parsed values on the web page
    values += `Parsed Floating Point Number Parts\n`;
    values += `    sign: ${data['sign']}\n`;
    values += ` decimal: ${data.decimal}\n`;
    values += `exponent: ${data.exponent}\n`;
  } else {
    values += `Invalid Floating Point Number\n`;
  }
  $('#values').html(values);
};
$(document).ready(() => {
  $('#parse').click(parseNumber);
  $('#string').val('+1.23E-10');
});
