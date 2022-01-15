all: test style lint
.PHONY: all

clean:
	@cargo clean
.PHONY: clean

build:
	@cargo build
.PHONY: build

doc:
	@cargo doc --all-features
.PHONY: doc

style:
	@rustup component add rustfmt --toolchain stable 2> /dev/null
	cargo fmt -- --check
.PHONY: style

format:
	@rustup component add rustfmt --toolchain stable 2> /dev/null
	cargo fmt
.PHONY: format

lint:
	@rustup component add clippy --toolchain stable 2> /dev/null
	cargo clippy --all-features --tests --all --examples -- -D clippy::all
.PHONY: lint

test: testall
.PHONY: test

checkall:
	cargo check --all-features
	cargo check --no-default-features
.PHONY: checkall

testall:
	cargo check --no-default-features --all
	cargo check --no-default-features --features test-support --all
	cargo test --all-features --all -- --nocapture
	cargo run --all-features --example join
	cargo run --all-features --example kill
	cargo run --all-features --example panic
	cargo run --all-features --example pool
	cargo run --all-features --example simple
	cargo run --all-features --example stdout
	cargo run --all-features --example timeout
	cargo run --all-features --example macro
	cargo run --all-features --example bad-serialization
	cargo run --all-features --example custom-serialization
	cargo run --all-features --example args -- 1 2 3
.PHONY: testall
