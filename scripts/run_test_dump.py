import subprocess, json
proc = subprocess.Popen(['cargo', 'test', '-p', 'iroha_core', '--test', 'contract_code_bytes', 'register_contract_code_bytes_stores_and_idempotent', '--', '--nocapture'], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
while True:
    line = proc.stdout.readline()
    if not line:
        break
    print(line.decode(), end='')
