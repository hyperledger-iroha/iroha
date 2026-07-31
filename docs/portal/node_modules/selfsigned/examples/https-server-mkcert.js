const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const selfsigned = require('../');

async function main() {
  // Get mkcert's CAROOT path
  let caroot;
  try {
    caroot = execSync('mkcert -CAROOT', { encoding: 'utf8' }).trim();
  } catch (err) {
    console.error('Error: mkcert is not installed or not in PATH');
    console.error('Install mkcert: https://github.com/FiloSottile/mkcert');
    process.exit(1);
  }

  const caKeyPath = path.join(caroot, 'rootCA-key.pem');
  const caCertPath = path.join(caroot, 'rootCA.pem');

  // Check if CA files exist
  if (!fs.existsSync(caKeyPath) || !fs.existsSync(caCertPath)) {
    console.error('Error: mkcert CA files not found');
    console.error('Run "mkcert -install" first to create the local CA');
    process.exit(1);
  }

  console.log('Using mkcert CA from:', caroot);

  // Read CA certificate and key
  const caKey = fs.readFileSync(caKeyPath, 'utf8');
  const caCert = fs.readFileSync(caCertPath, 'utf8');

  // Generate a certificate signed by mkcert's CA
  const pems = await selfsigned.generate([
    { name: 'commonName', value: 'localhost' }
  ], {
    days: 365,
    keySize: 2048,
    algorithm: 'sha256',
    ca: {
      key: caKey,
      cert: caCert
    }
  });

  // Create HTTPS server with the generated certificate
  const server = https.createServer({
    key: pems.private,
    cert: pems.cert
  }, (req, res) => {
    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end('Hello from HTTPS server with mkcert CA!\n');
  });

  const port = 3443;
  server.listen(port, () => {
    console.log(`HTTPS server running at https://localhost:${port}/`);
    console.log('Certificate fingerprint:', pems.fingerprint);
    console.log('\nSince this certificate is signed by mkcert\'s CA,');
    console.log('your browser should trust it automatically (if mkcert -install was run).');
    console.log('\nTest with: curl https://localhost:' + port);
  });
}

main().catch(console.error);
