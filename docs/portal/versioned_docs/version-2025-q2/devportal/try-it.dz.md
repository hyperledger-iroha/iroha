---
lang: dz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# དཔྱད་གཞིའི་བྱེམ་སྒམ་།

གོང་འཕེལ་གཏང་མི་ དྲྭ་ཚིགས་འདི་གིས་ གདམ་ཁ་ཅན་གྱི་ “Try it” ཀོན་སོལ་ཅིག་ བཏངམ་ཨིན།
ཡིག་ཆ་བཞག་མ་བཞག་པར་མཐའ་ཐིག་ཚུ། ཀོན་སོལ་ བརྒྱུད་འཕྲིན་གྱི་ཞུ་བ་ཚུ།
བརྡ་འཚོལ་ཚུ་གིས་ CORS གི་ཚད་གཞི་ཚུ་ བརྒལ་ཚུགས་པའི་སྐབས་ བརྡ་འཚོལ་གྱི་ ངོ་ཚབ་བརྒྱུད་དེ་ཨིན།
ཚད་གཞི་ཚད་གཞི་དང་བདེན་བཤད་བསྟར་སྤྱོད་འབད་ནི།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།

- Node.js 18.18 ཡང་ན་ གསརཔ་ (ཡོངས་འབྲེལ་གྱི་དགོས་མཁོ་ཚུ་དང་མཐུན་སྒྲིག་འབད།)
- Torii མཐའ་འཁོར་གནས་སྟངས་ལུ་ཡོངས་འབྲེལ་འཛུལ་སྤྱོད་འབད་ནི།
- ཁྱོད་ཀྱིས་ལག་ལེན་འཐབ་ནིའི་འཆར་གཞི་ཡོད་མི་ Torii གི་འགྲུལ་ལམ་ཟེར་སླབ་ཚུགས་པའི་ bearer token ཅིག།

པོརོག་སི་རིམ་སྒྲིག་ཆ་མཉམ་མཐའ་འཁོར་འགྱུར་ཅན་ཚུ་བརྒྱུད་དེ་འབདཝ་ཨིན། འོག་གི་ཐིག་ཁྲམ་
གལ་ཆེ་ཤོས་ཀྱི་ མཛུབ་མོ་ཚུ་ ཐོ་བཀོད་འབདཝ་ཨིན།

| འགྱུར་ཅན་ | དམིགས་ཡུལ། | སྔོན་སྒྲིག་ |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | གཞི་རྟེན་ Torii URL པོརོ་སི་གདོང་ཕྱོགས་ཚུ་གིས་ ཞུ་བ་འབདཝ་ཨིན། | *དགོས་མཁོ།** |
| `TRYIT_PROXY_LISTEN` | ས་གནས་གོང་འཕེལ་གྱི་དོན་ལུ་ཁ་བྱང་ཉན་ (རྩ་སྒྲིག་ `host:port` ཡང་ན་ `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | ངོ་ཚབ་ཟེར་སླབ་མི་ འབྱུང་ཁུངས་ཐོ་ཡིག་ | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` | སྔོན་སྒྲིག་འབག་མི་ཊོ་ཀེན་འདི་ Torii ལུ་སྤོ་བཤུད་འབདཝ་ཨིན། | _སྟོང་པ་_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | མཐའ་མཇུག་ལག་ལེན་པ་ཚུ་གིས་ `X-TryIt-Auth` བརྒྱུད་དེ་ རང་སོའི་རྟགས་མཚན་བཀྲམ་སྤེལ་འབད་བཅུག། | `0` |
| `TRYIT_PROXY_MAX_BODY` | མཐོ་ཤོས་ཞུ་བ་གཟུགས་ཚད་ (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | མི་ལི་སྐར་ཆ་ནང་ ཡར་འཕེལ་གྱི་དུས་ཚོད་བཏོན་ནི། | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | མཁོ་སྤྲོད་པ་རེ་ལུ་ ཆོག་ཐམ་བྱིན་ཆོག་པའི་ཞུ་བ་ཚུ་ IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | ཚད་གཞི་ (ms) གི་དོན་ལུ་ བཤུད་སྒོ་སྒྲིག་ (ms) | `60000` |

ངོ་ཚབ་འདི་གིས་ `GET /healthz` གིས་ བཀོད་སྒྲིག་འབད་ཡོད་པའི་ JSON འཛོལ་བ་ཚུ་སླར་ལོག་འབདཝ་ཨིནམ་དང་ དེ་ལས་ དང་ དེ་ལས་ གསལ་སྟོན་འབདཝ་ཨིན།
དྲན་ཐོ་ཨའུཊི་པུཊི་ལས་ རྡུལ་ཕྲན་བེ་ཊོ་ཀེན་ཚུ།

## ས་གནས་ནང་ ངོ་ཚབ་འགོ་བཙུགས།

ཁྱོད་ཀྱིས་ དྲྭ་ཚིགས་འདི་གཞི་སྒྲིག་འབད་བའི་སྐབས་ བརྟེན་པ་གཞི་བཙུགས་འབད་ནི།

```bash
cd docs/portal
npm install
```

ངོ་ཚབ་འདི་གཡོག་བཀོལ་ཞིནམ་ལས་ ཁྱོད་རའི་ Torii གི་དཔེ་ལུ་སྟོན་དགོ།

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

ཡིག་ཚུགས་འདི་གིས་ མཐའ་མཚམས་ཁ་བྱང་དང་ གདོང་ཕྱོགས་ཀྱི་ཞུ་བ་ཚུ་ `/proxy/*` ལས་ ནང་བསྐྱོད་འབདཝ་ཨིན།
རིམ་སྒྲིག་འབད་ཡོད་པའི་ Torii འབྱུང་ཁུངས་།

## དྲྭ་ཐག་ཝིཌི་གེཊསི།

ཁྱོད་ཀྱིས་ གོང་འཕེལ་གཏང་མི་ དྲྭ་ཚིགས་འདི་ བཟོ་བསྐྲུན་ ཡང་ན་ ཕྱག་ཞུ་བའི་སྐབས་ ཝིཌི་གེཊི་ཚུ་ལུ་ ཡུ་ཨར་ཨེལ་ གཞི་སྒྲིག་འབད།
པོརོག་སི་གི་དོན་ལུ་ལག་ལེན་འཐབ་དགོཔ་::

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

འོག་གི་ཆ་ཤས་ཚུ་གིས་ `docusaurus.config.js` ལས་ གནས་གོང་འདི་ཚུ་ལྷག་ཡོདཔ་ཨིན།

- **Swagger UI*** — `/reference/torii-swagger` ལུ་བཏོནམ་ཨིན། ཞུ་བ་ཅིག་ལག་ལེན་འཐབ་ཨིན།
  བེ་རི་སི་ཊར་ལུ་ བེ་ར་ཊོ་ཀེན་ཚུ་ རང་བཞིན་གྱིས་ མཉམ་སྦྲགས་འབདཝ་ཨིན།
- **RapiDoc** — `/reference/torii-rapidoc` ལུ་བཀོད་ཡོདཔ། མེ་ལོང་གིས་ ཊོ་ཀན་གྱི་ས་སྒོ།
  དང་ ངོ་ཚབ་ལུ་ འབད་རྩོལ་བསྐྱེད་མི་ ཞུ་བ་ཚུ་ལུ་ རྒྱབ་སྐྱོར་འབདཝ་ཨིན།
- **དེ་ མཉམ་བསྡོམས་** — ཨེ་པི་ཨའི་ སྤྱི་མཐོང་ཤོག་ལེབ་གུ་བཙུགས་ཡོདཔ་ཨིན། ཁྱོད་ཀྱིས་ སྲོལ་སྒྲིག་གཏངམ་ཨིན།
  ཞུ་བ་དང་ མགོ་ཡིག་བལྟ་ནི་ དེ་ལས་ ལན་འདེབས་ཚོགས་སྡེ་ཚུ་ བརྟག་དཔྱད་འབད།

ཝིཌི་གེཊི་གང་རུང་ཅིག་ནང་ ཊོ་ཀེན་འདི་བསྒྱུར་བཅོས་འབད་མི་འདི་གིས་ ད་ལྟོའི་བརྡ་འཚོལ་ལཱ་ཡུན་ལུ་རྐྱངམ་ཅིག་ ཕན་གནོད་ཡོདཔ་ཨིན། ཚིག༌ཕྲད
པོརོག་སི་ནམ་ཡང་མ་གནས་པ་ ཡང་ན་ བཀྲམ་སྤེལ་འབད་ཡོད་པའི་བརྡ་མཚོན་འདི་ དྲན་ཐོ་བཀོད།

## བལྟ་རྟོག་དང་བཀོལ་སྤྱོད།

ཞུ་བ་རེ་རེ་བཞིན་དུ་ ཐབས་ལམ་དང་ འགྲུལ་ལམ་ འབྱུང་ཁུངས་ ཡར་འགྲོས་གནས་རིམ་ དེ་ལས་ ནང་བསྐྱོད་འབད་ཡོདཔ་ཨིན།
བདེན་བཤད་འབྱུང་ཁུངས་ (`override`, `default`, ཡང་ན་ `client`). ཊོ་ཀེན་ཚུ་ནམ་ཡང་མེན།
གསོག་འཇོག་འབད་ཡོདཔ་—བེ་ཡར་མགོ་ཡིག་དང་ `X-TryIt-Auth` གནས་གོང་ཚུ་ ཧེ་མ་གི་ཧེ་མ་ལས་ བསྒྱུར་བཅོས་འབད་ཡོདཔ་ཨིན།
ནང་བསྐྱོད་—དེ་ལས་ ཁྱོད་ཀྱིས་ stdout འདི་ ཚ་གྱང་མ་ལང་པར་ ལྟེ་བ་བསྡུ་གསོག་འབད་མི་ཅིག་ལུ་ གདོང་བསྐྱོད་འབད་ཚུགས།
།གསང་བ་འཛེགས་པ།

### གསོ་བའི་ཞིབ་དཔྱད་དང་ཉེན་བརྡ།བཀྲམ་སྤེལ་འབད་བའི་སྐབས་ ཡང་ན་ ལས་རིམ་ཅིག་གུ་ ཡང་ན་ ལས་རིམ་ཅིག་གུ་ བསྡུ་སྒྲིག་འབད་ཡོད་པའི་ འཚོལ་ཞིབ་འདི་ གཡོག་བཀོལ།

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

ཁོར་ཡུག་གི་མཛུབ་མོ་།

- `TRYIT_PROXY_SAMPLE_PATH` — གདམ་ཁའི་ Torii གི་ལམ་ (`/proxy` མེད་པར་) ལུས་སྦྱོང་འབད་ནིའི་དོན་ལུ་ཨིན།
- `TRYIT_PROXY_SAMPLE_METHOD` — `GET` ལུ་སྔོན་སྒྲིག་འབདཝ་ཨིན། འབྲི་ནིའི་ལམ་ཚུ་གི་དོན་ལུ་ `POST` ལུ་གཞི་སྒྲིག་འབད་ཡོདཔ།
- `TRYIT_PROXY_PROBE_TOKEN` — དཔེ་ཚད་འབོད་བརྡ་གི་དོན་ལུ་ གནས་སྐབས་ཀྱི་ བེ་ཊར་ཊོ་ཀེན་ཅིག་ བཙུགསཔ་ཨིན།
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — སྔོན་སྒྲིག་ ༥ཨེསི་དུས་ཚོད་བཏོན་ཡོདཔ་ཨིན།
- `TRYIT_PROXY_PROBE_METRICS_FILE` — གདམ་ཁའི་ Prometheus ཚིག་ཡིག་ཡིག་སྣོད་ཀྱི་འགྲོ་ཡུལ་ `probe_success`/Torii.
- `TRYIT_PROXY_PROBE_LABELS` — ལྷོད་རྟགས་ཁ་ཕྱེ་ཡོད་པའི་ `key=value` ཆ་ཚུ་ མེ་ཊིག་ཚུ་ལུ་ མཐུད་ཡོདཔ་ཨིན། (`job=tryit-proxy` དང་ `instance=<proxy URL>`)

གྲུབ་འབྲས་ཚུ་ ཡིག་ཐོག་ལུ་བཀོད་ཡོད་པའི་ ཞིབ་དཔྱད་འདི་ གསལ་བཏོན་འབད་དེ་ ཚིག་ཡིག་ཡིག་སྣོད་བསྡུ་སྒྲིག་འབད་མི་ཅིག་ལུ་ ལྟོ་བྱིན།
ལམ་ (དཔེར་ན་ `/var/lib/node_exporter/textfile_collector/tryit.prom`) དང་།
སྲོལ་སྒྲིག་ཁ་ཡིག་གང་རུང་ཅིག་ཁ་སྐོང་བརྐྱབ།

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

ཡིག་ཚུགས་འདི་གིས་ མེ་ཊིགསི་ཡིག་སྣོད་ཚུ་ རྡུལ་ཕྲན་གྱི་ཐོག་ལས་ ལོག་བྲིས་དོ་ཡོདཔ་ལས་ ཁྱོད་རའི་བསྡུ་གསོག་པ་གིས་ ཨ་རྟག་ར་ a ལྷགཔ་ཨིན།
ཆ་ཚང་སྤྲོད་ལེན་ཆ་ཚང་།

ལྗིད་ཚད་མར་ཕབ་ཀྱི་ཉེན་བརྡ་གི་དོན་ལུ་ ཁྱོད་རའི་ལྟ་རྟོག་བང་རིམ་ནང་ལུ་ འཚོལ་ཞིབ་འདི་ གློག་ཐག་འབད། A I 18NT00000001X
དཔེ་འབད་བ་ཅིན་ རིམ་མཐུན་འཐུས་ཤོར་གཉིས་ཀྱི་ཤུལ་ལས་ ཤོག་ལེབ་ཚུ་:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### བསྐོར་རྒྱབ་རང་འགུལ།

དམིགས་གཏད་ Torii URL དུས་མཐུན་བཟོ་ནིའི་དོན་ལུ་ འཛིན་སྐྱོང་གྲོགས་རམ་པ་ལག་ལེན་འཐབ། ༡ ཡིག་གཟུགས་འདི།
ཧེ་མའི་རིམ་སྒྲིག་འདི་ `.env.tryit-proxy.bak` ནང་ལུ་གསོག་འཇོག་འབདཝ་ཨིན།
བརྡ་བཀོད་རྐྱང་པ།

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

ཁྱོད་ཀྱི་བཀྲམ་སྤེལ་འབད་བ་ཅིན་ `--env` ཡང་ན་ `TRYIT_PROXY_ENV` དང་ཅིག་ཁར་ env ཡིག་སྣོད་འགྲུལ་ལམ་འདི་བཀག་བཞག།
རིམ་སྒྲིག་གཞན་ཁར་གསོག་འཇོག་འབདཝ་ཨིན།