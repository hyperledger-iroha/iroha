const parseUri = () => {
  // get the input string (the number to parse) from the web page
  const inputUri = $('#uri').val();

  // parse the input string
  const result = uriParser.parse(inputUri);

  // display the parser's result on the web page
  if (result) {
    $('#result').html('Result\n' + JSON.stringify(result, null, 2));
  } else {
    $('#result').html('Result\ninvalid URI');
  }
};
$(document).ready(() => {
  $('#parse').click(parseUri);
  $('#uri').val('https://user:pass@example.com:123/one/two.three?q1=a1&q2=a2#body');
});
