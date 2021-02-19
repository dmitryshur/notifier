export RUST_LOG := "INFO"
export API_ADDRESS := "0.0.0.0:4000"
export RABBIT_ADDRESS := "amqp://0.0.0.0:5672"

run-all:
  ./run.sh

run BIN:
  cargo run --bin {{BIN}}

test:
  cargo test

check:
  cargo clippy

scraper:
  yarn --cwd scraper/ start

