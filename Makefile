.PHONY: sync-status sync-pull sync-push sync-init ci

sync-status:
	./python sync_wgpu.py status

sync-pull:
	./python sync_wgpu.py pull

sync-push:
	./python sync_wgpu.py push

sync-init:
	uv sync

# Local CI: mirrors the GitHub Actions pipeline for pre-push validation.
ci:
	@echo "=== Format check ==="
	cargo fmt --all -- --check
	@echo "=== Build (default) ==="
	cargo build --workspace
	@echo "=== Build (test_mode) ==="
	cargo build --workspace --features test_mode
	@echo "=== Clippy ==="
	cargo clippy --workspace -- -D warnings
	@echo "=== Clippy (test_mode) ==="
	cargo clippy --workspace --features test_mode -- -D warnings
	@echo "=== Tests ==="
	cargo test --workspace --no-fail-fast
	@echo "=== Feature gate: tracy ==="
	cargo check --workspace --features tracy
	@echo "=== CI passed ==="
