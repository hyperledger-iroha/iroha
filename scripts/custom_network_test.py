#!/usr/bin/env python3
import time
import subprocess
import threading
import sys
import urllib.request
import json
import os

# Configuration
PEERS = 7
TX_INTERVAL = 0.5  # seconds
TARGET_BLOCKS = 100
OUT_DIR = os.path.abspath("target/tmp_localnet")
CLIENT_CONFIG = os.path.join(OUT_DIR, "client.toml")
STATUS_URL = "http://127.0.0.1:29080/status"

def deploy_network():
    print(f"Deploying localnet with {PEERS} peers to {OUT_DIR}...")
    cmd = [
        "bash", "scripts/deploy_localnet.sh",
        "--peers", str(PEERS),
        "--out-dir", OUT_DIR,
        "--block-time-ms", "4000",
        "--commit-time-ms", "8000",
        "--force"
    ]
    subprocess.check_call(cmd)

def get_status():
    try:
        with urllib.request.urlopen(STATUS_URL, timeout=2) as response:
            return json.loads(response.read().decode('utf-8'))
    except Exception as e:
        print(f"Error fetching status: {e}")
        return None

def send_transactions(stop_event):
    print("Starting transaction generator...")
    # Find iroha binary
    # deploy_localnet.sh builds binaries in target/debug or release. 
    # Default is debug.
    iroha_bin = "target/debug/iroha"
    
    # Check if binary exists
    if not os.path.exists(iroha_bin):
         print(f"Warning: {iroha_bin} not found. Trying to find it...")
         # Try to find it in release
         if os.path.exists("target/release/iroha"):
             iroha_bin = "target/release/iroha"
         else:
             print("Error: iroha binary not found.")
             return

    print(f"Using iroha binary: {iroha_bin}")

    # We use "transaction ping"
    count = 0
    while not stop_event.is_set():
        try:
            cmd = [
                iroha_bin,
                "--config", CLIENT_CONFIG,
                "transaction", "ping",
                "--msg", f"ping-{time.time()}"
            ]
            
            # Capture output
            result = subprocess.run(cmd, capture_output=True, text=True)
            if result.returncode != 0:
                print(f"Transaction failed: {result.stderr}")
            elif count % 10 == 0:
                 print(f"Sent ping {count}...")
            count += 1
            
        except Exception as e:
            print(f"Error sending transaction: {e}")
        
        time.sleep(TX_INTERVAL)

def monitor_network():
    print("Monitoring network...")
    initial_status = get_status()
    if not initial_status:
        print("Could not get initial status. Aborting.")
        return False

    initial_view_changes = initial_status.get("view_changes", 0)
    print(f"Initial view changes: {initial_view_changes}")

    last_block = initial_status.get("blocks", 0)
    last_change_time = time.time()
    
    stop_tx = threading.Event()
    tx_thread = threading.Thread(target=send_transactions, args=(stop_tx,))
    tx_thread.start()

    success = False
    try:
        while True:
            status = get_status()
            if not status:
                time.sleep(1)
                continue

            blocks = status.get("blocks", 0)
            view_changes = status.get("view_changes", 0)
            
            print(f"Height: {blocks}, View Changes: {view_changes}")

            if view_changes > initial_view_changes:
                print(f"FAILURE: View changes increased! Initial: {initial_view_changes}, Current: {view_changes}")
                # We fail immediately if view changes happen, per requirement "without ... view changes"
                # If the requirement allows *some* view changes during startup, we might need to be lenient, 
                # but "without hanging or view changes" usually implies stability.
                # However, sometimes network startup causes 1 view change. 
                # Let's be strict for now as per instructions.
                break

            if blocks > last_block:
                last_block = blocks
                last_change_time = time.time()
            elif time.time() - last_change_time > 20: # 20 seconds without block increase
                print("FAILURE: Network seems hung. No new blocks for 20 seconds.")
                break

            if blocks >= TARGET_BLOCKS:
                print("SUCCESS: Reached target block height.")
                success = True
                break

            time.sleep(1)

    except KeyboardInterrupt:
        print("Interrupted.")
    finally:
        stop_tx.set()
        tx_thread.join()
        stop_network()

    return success

def stop_network():
    print("Stopping network...")
    stop_script = os.path.join(OUT_DIR, "stop.sh")
    if os.path.exists(stop_script):
        subprocess.run(["bash", stop_script], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

if __name__ == "__main__":
    deploy_network()
    if monitor_network():
        sys.exit(0)
    else:
        sys.exit(1)
