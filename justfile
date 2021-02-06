run-all:
  ./run.sh

run BIN:
  cargo run --bin {{BIN}}

test:
  cargo test

check:
  cargo check
