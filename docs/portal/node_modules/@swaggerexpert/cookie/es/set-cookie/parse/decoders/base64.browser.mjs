const base64Decoder = input => {
  const binaryString = atob(input);
  const bytes = new Uint8Array([...binaryString].map(char => char.charCodeAt(0)));
  return new TextDecoder().decode(bytes);
};
export default base64Decoder;