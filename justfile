export API_PORT := "4000"
export RUST_LOG := "INFO"

run-all:
  ./run.sh

run BIN:
  cargo run --bin {{BIN}}

test:
  cargo test

check:
  cargo clippy

