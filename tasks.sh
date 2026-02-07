#!/bin/bash
# GLASS Project - Task Runner

case "$1" in
    sync-status)
        ./python sync_wgpu.py status
        ;;
    sync-pull)
        ./python sync_wgpu.py pull
        ;;
    sync-push)
        ./python sync_wgpu.py push
        ;;
    sync-init)
        uv sync
        ;;
    *)
        echo "Usage: ./tasks.sh {sync-status|sync-pull|sync-push|sync-init}"
        exit 1
        ;;
esac
