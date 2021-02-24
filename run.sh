#!/bin/bash

trap 'kill %1' SIGINT

cargo run --bin api | sed -e 's/^/[api] /' &
cargo run --bin scheduler | sed -e 's/^/[scheduler] /' &
cargo run --bin bot | sed -e 's/^/[bot] /' &
yarn --cwd scraper/ start
