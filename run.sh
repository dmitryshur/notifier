#!/bin/bash
 
trap 'kill %1' SIGINT
 
cargo run --bin api | sed -e 's/^/[api] /' &
cargo run --bin scheduler | sed -e 's/^/[scheduler] /'
