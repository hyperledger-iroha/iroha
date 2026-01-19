import time
import subprocess
import json
import sys

API_URL = "http://127.0.0.1:8080"
CONFIG = "/tmp/iroha-localnet-7peer/client.toml"
BINARY = "target/debug/iroha"

def get_status():
    try:
        # Use curl to avoid python requests dependency if not installed, or urllib
        res = subprocess.run(["curl", "-s", f"{API_URL}/status"], capture_output=True, text=True)
        if res.returncode != 0 or not res.stdout.strip(): return None
        try:
            return json.loads(res.stdout)
        except json.JSONDecodeError:
            return None
    except:
        return None

def send_ping():
    subprocess.run([BINARY, "--config", CONFIG, "transaction", "ping", "--count", "1", "--no-wait"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

print("Waiting for network...")
ready = False
for i in range(60):
    status = get_status()
    if status and "blocks" in status:
        print(f"Network ready. Blocks: {status['blocks']}")
        ready = True
        break
    time.sleep(1)

if not ready:
    print("Network failed to start.")
    sys.exit(1)

print("Starting load...")
start_time = time.time()
target_height = 100
while True:
    elapsed = time.time() - start_time
    if elapsed > 300:
        print("Timeout reached.")
        sys.exit(1)
    
    status = get_status()
    height = status["blocks"] if status else 0
    print(f"Height: {height}, Elapsed: {elapsed:.1f}s")
    
    if height >= target_height:
        print(f"Success! Reached height {height} in {elapsed:.1f}s")
        break
        
    send_ping()
    time.sleep(0.5)
