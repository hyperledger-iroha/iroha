import time
import subprocess
import json
import urllib.request
import sys
import os

OUT_DIR = "target/tmp_localnet"
CLIENT_CONFIG = os.path.join(OUT_DIR, "client.toml")
IROHA_BIN = "target/debug/iroha"
STATUS_URL = "http://127.0.0.1:29080/status"

def get_status():
    try:
        with urllib.request.urlopen(STATUS_URL, timeout=2) as response:
            return json.loads(response.read().decode())
    except Exception as e:
        print(f"Error fetching status: {e}")
        return None

def send_tx():
    cmd = [
        IROHA_BIN,
        "--config", CLIENT_CONFIG,
        "transaction", "ping",
        "--count", "1",
        "--no-wait"
    ]
    # Suppress output to keep logs clean, unless error
    try:
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, check=True)
    except subprocess.CalledProcessError as e:
        print(f"Failed to send transaction: {e.stderr.decode()}")

def main():
    print("Starting simulation...")
    
    # Wait for initial connection
    for _ in range(10):
        status = get_status()
        if status:
            break
        time.sleep(1)
    
    if not status:
        print("Could not connect to peer.")
        sys.exit(1)
        
    start_blocks = status.get('blocks', 0)
    # The output from deploy_localnet showed "view_changes": 0.
    initial_view_changes = status.get('view_changes', 0)
    print(f"Initial State: Blocks={start_blocks}, View Changes={initial_view_changes}")

    target_height = start_blocks + 100
    print(f"Target Height: {target_height}")
    
    start_time = time.time()
    
    while True:
        status = get_status()
        if not status:
            print("Lost connection!")
            sys.exit(1)
            
        current_blocks = status.get('blocks', 0)
        view_changes = status.get('view_changes', 0)
        
        # Simple progress log every 10 blocks or so to avoid spam
        if current_blocks % 10 == 0:
            print(f"Current: Blocks={current_blocks}, View Changes={view_changes}")
        
        if view_changes > initial_view_changes:
            print(f"FAILURE: View changes increased: {view_changes} (started at {initial_view_changes})")
            sys.exit(1)
            
        if current_blocks >= target_height:
            print(f"SUCCESS: Reached target height {target_height} (Current: {current_blocks})")
            break
            
        send_tx()
        
        # Check timeout (e.g. 200 seconds for 100 blocks should be ample time given 1s block time)
        if time.time() - start_time > 300:
             print("FAILURE: Timeout reached.")
             sys.exit(1)

        time.sleep(0.5)

if __name__ == "__main__":
    main()
