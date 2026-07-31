/* eslint-disable func-names */
/* eslint-disable new-cap */
/* eslint-disable no-undef */
/*  *************************************************************************************
 *   copyright: Copyright (c) 2023 Lowell D. Thomas, all rights reserved
 *     license: BSD-2-Clause (https://opensource.org/licenses/BSD-2-Clause)
 *   ********************************************************************************* */
// This is a demonstration of the bare minimum needed to set up a parser
// and parse a given input string.
//
// It is the same "`simple`" example but modified to use `apgLib.js` and run in a browser web page.
// It works with the companion file `browser.html`.
const doTrace = function doTrace() {
  const parser = new Parser();
  const grammar = new Grammar();
  const id = identifiers;
  const utils = utilities;
  parser.stats = new Stats();
  parser.trace = new Trace();

  /* use a hard-coded input string for this example */
  const inputString = $('#string').val();

  /* set the parser's "start rule" */
  const startRule = 'S';

  // clear any previous trace and stats info
  $('#tabs-stats').html('');
  $('#tabs-trace').html('');
  $('#values').html('');

  // This is the call that will finally parse the input string.
  const result = parser.parse(grammar, startRule, inputString);

  /* display parser results */
  $('#result').html('Parser Result<br>' + JSON.stringify(result));

  // This section will demonstrate all of the options for the display of the
  // parsing statistics. Finally, all options will be displayed
  // on a single web page. See the page `html/simple-stats.html` for the results.
  values = '\nClick the Stats tab to see the operator and rule name hit count statistics.\n';
  values += 'Click the Trace tab to see a full trace of the parser through the parse tree.\n';
  $('#values').html(values);

  // add the stats to the stats tab
  $('#tabs-stats').html(parser.stats.displayStats());
  $('#tabs-stats').append('<br>');
  $('#tabs-stats').append(parser.stats.displayHits());
  $('#tabs-stats').append('<br>');
  $('#tabs-stats').append(parser.stats.displayHits('a'));
  $('#tabs-stats').append('<br>');
  $('#tabs-stats').append(parser.stats.displayHits('i'));

  // add the trace to the trace tab
  let t = parser.trace.displayTrace();
  $('#tabs-trace').html(t);
};
$(document).ready(() => {
  const con = new Grammar();
  $('#grammar-bnf').html(con.toString());
  $('#parse').click(doTrace);
  $('#string').val('aaabbbccc');
  $('#tabs').tabs();
});
