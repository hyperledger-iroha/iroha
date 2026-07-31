const parser = new Parser();
const grammar = new Grammar();
const id = identifiers;
const utils = utilities;

// display the ABNF grammar on the web page
$('#grammar-bnf').html(grammar.toString());

// define the parser's callback functions
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
const parseNumber = () => {
  // get the input string (the number to parse) from the web page
  const inputString = $('#string').val();

  // parse the input string
  const result = parser.parse(grammar, 'start', inputString, data);

  // display the parser's result on the web page
  $('#result').html('Parser Result<br>' + JSON.stringify(result));
  let values = '';
  if (result.success) {
    // display the parsed values on the web page
    values += `Parsed Statments\n`;
    values += `statement1: ${data['statement1']}\n`;
    values += `statement2: ${data['statement2']}\n`;
  } else {
    values += `One or more invalid statements.\n`;
  }
  $('#values').html(values);
};
$(document).ready(() => {
  $('#parse').click(parseNumber);
  let v = 'See you later alligator. \uD83D\uDE01\n';
  v += 'After while crocodile. \uD83D\uDE0E\n';
  $('#string').val(v);
});
