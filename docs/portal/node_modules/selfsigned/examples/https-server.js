const https = require('https');
const selfsigned = require('../');

async function main() {
  // Generate a self-signed certificate
  const pems = await selfsigned.generate([
    { name: 'commonName', value: 'localhost' }
  ], {
    days: 365,
    keySize: 2048,
    algorithm: 'sha256'
  });

  // Create HTTPS server with the generated certificate
  const server = https.createServer({
    key: pems.private,
    cert: pems.cert
  }, (req, res) => {
    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end('Hello from self-signed HTTPS server!\n');
  });

  const port = 3443;
  server.listen(port, () => {
    console.log(`HTTPS server running at https://localhost:${port}/`);
    console.log('Certificate fingerprint:', pems.fingerprint);
    console.log('\nNote: Your browser will warn about the self-signed certificate.');
    console.log('Test with: curl -k https://localhost:' + port);
  });
}

main().catch(console.error);
