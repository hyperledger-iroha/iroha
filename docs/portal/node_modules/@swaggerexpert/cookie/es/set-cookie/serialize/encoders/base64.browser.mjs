const base64Encoder = input => {
  const bytes = new TextEncoder().encode(input);
  const binaryString = bytes.reduce((acc, byte) => acc + String.fromCharCode(byte), '');
  return btoa(binaryString);
};
export default base64Encoder;