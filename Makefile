.PHONY: build build-release test lint fmt fmt-check check install clean hooks release

build:
	cargo build

build-release:
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

hooks:
	git config core.hooksPath .githooks
	@echo "pre-commit hook enabled (.githooks)"

# Usage: make release VERSION=0.8.0
# Runs checks, bumps Cargo.toml/Cargo.lock, commits, tags vVERSION, and pushes
# branch + tag together, which triggers the GitHub release workflow.
release:
	@[ -n "$(VERSION)" ] || { echo "usage: make release VERSION=X.Y.Z"; exit 1; }
	@echo "$(VERSION)" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$$' || { echo "error: VERSION must be X.Y.Z (no leading v), got '$(VERSION)'"; exit 1; }
	@[ -z "$$(git status --porcelain)" ] || { echo "error: working tree not clean"; git status --short; exit 1; }
	@! git rev-parse -q --verify "refs/tags/v$(VERSION)" >/dev/null || { echo "error: tag v$(VERSION) already exists"; exit 1; }
	$(MAKE) check
	sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml && rm -f Cargo.toml.bak
	cargo update --workspace
	git add Cargo.toml Cargo.lock
	git commit -m "release: v$(VERSION)"
	git tag "v$(VERSION)"
	git push --atomic origin HEAD "v$(VERSION)"
	@echo "released v$(VERSION) — release workflow triggered"
