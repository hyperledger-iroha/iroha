#!/bin/bash
set -ex
cd test_docker
sleep 10
./iroha_client_cli asset get --account_id alice@wonderland --id rose#wonderland | grep -q 'Quantity(13)'
