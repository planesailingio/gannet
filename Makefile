.PHONY: build release test lint fmt fmt-check check install clean

build:
	cargo build

release:
	cargo build --release

test:
	cargo test

lint:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

check: fmt-check lint test

install:
	cargo install --path .

clean:
	cargo clean
