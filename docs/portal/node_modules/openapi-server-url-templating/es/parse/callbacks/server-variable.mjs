import { identifiers, utilities } from 'apg-lite';
const serverVariable = (state, chars, phraseIndex, phraseLength, data) => {
  if (state === identifiers.SEM_PRE) {
    if (Array.isArray(data) === false) {
      throw new Error("parser's user data must be an array");
    }
    data.push(['server-variable', utilities.charsToString(chars, phraseIndex, phraseLength)]);
  }
  return identifiers.SEM_OK;
};
export default serverVariable;