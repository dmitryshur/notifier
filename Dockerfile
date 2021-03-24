FROM rust:1.50.0 AS rust-dev-builder
# Copy workspace files
WORKDIR /app
COPY Cargo.toml .
COPY Cargo.lock .

# Create the members of the workspace and copy their Cargo.toml files
RUN USER=root cargo new api
RUN USER=root cargo new bot
RUN USER=root cargo new broker
RUN USER=root cargo new scheduler

WORKDIR /app/api
COPY api/Cargo.toml .

WORKDIR /app/bot
COPY bot/Cargo.toml .

WORKDIR /app/broker
COPY broker/Cargo.toml .

WORKDIR /app/scheduler
COPY scheduler/Cargo.toml .

# Cache workspace dependencies
WORKDIR /app
RUN cargo build

FROM rust-dev-builder as api-dev
EXPOSE 4000
ENTRYPOINT ["/usr/local/cargo/bin/cargo", "run", "--bin", "api"]

FROM rust-dev-builder as bot-dev
ENTRYPOINT ["/usr/local/cargo/bin/cargo", "run", "--bin", "bot"]

FROM rust-dev-builder as scheduler-dev
ENTRYPOINT ["/usr/local/cargo/bin/cargo", "run", "--bin", "scheduler"]

FROM node:14.15.0 AS node-builder
WORKDIR /app
COPY scraper/package.json .
COPY scraper/yarn.lock .
COPY scraper/tsconfig.json .
COPY scraper/babel.config.js .
RUN yarn
COPY scraper/src/ src/

FROM node-builder as scraper
ENTRYPOINT yarn start

FROM rust:1.50.0 AS rust-prod-builder
# Copy workspace files
WORKDIR /app
COPY Cargo.toml .
COPY Cargo.lock .

# Create the members of the workspace and copy their Cargo.toml files
RUN USER=root cargo new api
RUN USER=root cargo new bot
RUN USER=root cargo new broker
RUN USER=root cargo new scheduler

WORKDIR /app/api
COPY api/Cargo.toml .

WORKDIR /app/bot
COPY bot/Cargo.toml .

WORKDIR /app/broker
COPY broker/Cargo.toml .

WORKDIR /app/scheduler
COPY scheduler/Cargo.toml .

# Cache workspace dependencies
WORKDIR /app
RUN cargo build --release

FROM rust-prod-builder AS rust-prod
RUN rm api/src/*.rs
RUN rm bot/src/*.rs
RUN rm broker/src/*.rs
RUN rm scheduler/src/*.rs
COPY api/src/ api/src/
COPY bot/src/ bot/src/
COPY broker/src/ broker/src/
COPY scheduler/src/ scheduler/src/
RUN cargo build --release

# TODO document in readme about incompatible ssl versions
FROM rust:1.50.0 AS api-prod
WORKDIR /app
EXPOSE 4000
COPY --from=rust-prod /app/target/release/api ./api
RUN ls /
RUN ls /app
ENTRYPOINT ["/app/api"]

FROM rust:1.50.0 AS bot-prod
WORKDIR /app
COPY --from=rust-prod /app/target/release/bot ./bot
ENTRYPOINT ["/app/bot"]

FROM rust:1.50.0 AS scheduler-prod
WORKDIR /app
COPY --from=rust-prod /app/target/release/scheduler ./scheduler
ENTRYPOINT ["/app/scheduler"]
