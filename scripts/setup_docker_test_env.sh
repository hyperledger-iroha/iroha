#!/bin/bash
mkdir test_docker
cp ./target/debug/iroha_client_cli test_docker
cp ./iroha_client/config.json test_docker
cp ./scripts/metadata.json test_docker
docker-compose up -d --force-recreate
sleep 10
