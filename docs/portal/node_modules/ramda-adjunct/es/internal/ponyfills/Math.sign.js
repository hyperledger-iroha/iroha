var signPonyfill = function signPonyfill(number) {
  return (number > 0) - (number < 0) || +number;
};
export default signPonyfill;