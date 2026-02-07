.PHONY: sync-status sync-pull sync-push sync-init

sync-status:
./python sync_wgpu.py status

sync-pull:
./python sync_wgpu.py pull

sync-push:
./python sync_wgpu.py push

sync-init:
uv sync
