#!/usr/bin/env python3
"""sync_wgpu.py — Git Subtree helper for third_party/wgpu in GLASS.

Commands
--------
  setup   Add the wgpu remote and perform the initial subtree add (or adopt).
  pull    Fetch upstream wgpu changes into third_party/wgpu (--squash).
  push    Push local wgpu-hal patches back to the fork.
  status  Show current remote and subtree state.

Usage
-----
  python sync_wgpu.py setup
  python sync_wgpu.py pull
  python sync_wgpu.py push
  python sync_wgpu.py status
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

# ── Configuration ────────────────────────────────────────────────────────────
GLASS_ORIGIN = "https://github.com/RomainROCH/GLASS-UltimateOverlay.git"
WGPU_FORK_URL = "https://github.com/RomainROCH/wgpu.git"
WGPU_REMOTE = "wgpu-fork"          # name of the git remote for the fork
SUBTREE_PREFIX = "third_party/wgpu"
DEFAULT_BRANCH = "glass-patch-v24.0.4"  # branch on the wgpu fork with our patches
# ─────────────────────────────────────────────────────────────────────────────


def run(
    cmd: list[str],
    *,
    check: bool = True,
    capture: bool = False,
    dry_run: bool = False,
) -> subprocess.CompletedProcess[str]:
    """Run a shell command with logging."""
    print(f"  → {' '.join(cmd)}")
    if dry_run:
        return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")
    return subprocess.run(
        cmd,
        check=check,
        text=True,
        capture_output=capture,
    )


def git(*args: str, check: bool = True, capture: bool = False) -> subprocess.CompletedProcess[str]:
    """Shorthand for running a git command."""
    return run(["git", *args], check=check, capture=capture)


def repo_root() -> Path:
    """Return the repo root, or abort if not inside a git repo."""
    result = git("rev-parse", "--show-toplevel", capture=True, check=False)
    if result.returncode != 0:
        abort("Not inside a git repository.")
    return Path(result.stdout.strip())


def abort(msg: str) -> None:
    print(f"\n✗ ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def ensure_clean_worktree() -> None:
    """Abort if the working tree has uncommitted changes."""
    result = git("status", "--porcelain", capture=True)
    if result.stdout.strip():
        abort(
            "Working tree is dirty. Commit or stash your changes first.\n"
            f"  Dirty files:\n{result.stdout}"
        )


def remote_exists(name: str) -> bool:
    result = git("remote", capture=True)
    return name in result.stdout.splitlines()


def current_origin_url() -> str | None:
    result = git("remote", "get-url", "origin", capture=True, check=False)
    if result.returncode == 0:
        return result.stdout.strip()
    return None


# ── Commands ─────────────────────────────────────────────────────────────────

def cmd_status() -> None:
    """Show current remote and subtree state."""
    print("\n=== sync_wgpu status ===\n")

    # Remotes
    print("Remotes:")
    git("remote", "-v")

    # Check subtree merge commits
    print(f"\nSubtree merge history for '{SUBTREE_PREFIX}':")
    result = git(
        "log", "--oneline", "--all", "--grep", f"git-subtree-dir: {SUBTREE_PREFIX}",
        capture=True, check=False,
    )
    if result.stdout.strip():
        print(result.stdout)
    else:
        print("  (no subtree merge commits found — directory may be manually managed)")

    # Check if prefix exists
    prefix_path = repo_root() / SUBTREE_PREFIX
    if prefix_path.is_dir():
        file_count = sum(1 for _ in prefix_path.rglob("*") if _.is_file())
        print(f"\n'{SUBTREE_PREFIX}' exists on disk with {file_count} files.")
    else:
        print(f"\n'{SUBTREE_PREFIX}' does NOT exist on disk.")


def cmd_setup() -> None:
    """Configure remotes and initialise the subtree."""
    print("\n=== sync_wgpu setup ===\n")
    ensure_clean_worktree()

    root = repo_root()

    # ── Step 1: Fix origin if it points to the wgpu fork instead of GLASS ──
    origin_url = current_origin_url()
    if origin_url and "wgpu" in origin_url.lower() and "GLASS" not in origin_url:
        print(f"⚠ origin currently points to wgpu fork: {origin_url}")
        print(f"  Fixing origin → {GLASS_ORIGIN}")
        git("remote", "set-url", "origin", GLASS_ORIGIN)

    # ── Step 2: Add wgpu-fork remote ────────────────────────────────────────
    if remote_exists(WGPU_REMOTE):
        print(f"Remote '{WGPU_REMOTE}' already exists, updating URL.")
        git("remote", "set-url", WGPU_REMOTE, WGPU_FORK_URL)
    else:
        print(f"Adding remote '{WGPU_REMOTE}' → {WGPU_FORK_URL}")
        git("remote", "add", WGPU_REMOTE, WGPU_FORK_URL)

    git("fetch", WGPU_REMOTE)

    # ── Step 3: Subtree add (or adopt existing directory) ───────────────────
    prefix_path = root / SUBTREE_PREFIX
    if prefix_path.is_dir():
        # Directory already exists — we need to adopt it as a subtree.
        # Strategy: remove the dir from index + disk, commit, then subtree add,
        # then verify the build still works.
        print(f"\n'{SUBTREE_PREFIX}' already exists. Adopting as subtree…")
        print("  Step 3a: Removing existing directory from git index…")
        git("rm", "-r", "--cached", SUBTREE_PREFIX)

        # Also remove untracked wgpu files from disk so subtree add can write cleanly
        import shutil
        if prefix_path.exists():
            shutil.rmtree(prefix_path)
            print(f"  Step 3b: Removed '{SUBTREE_PREFIX}' from disk.")

        git("commit", "-m", "chore: remove manually-managed wgpu before subtree adoption")

        print("  Step 3c: Adding subtree from wgpu-fork…")
        git(
            "subtree", "add",
            "--prefix", SUBTREE_PREFIX,
            WGPU_REMOTE, DEFAULT_BRANCH,
            "--squash",
            "-m", f"chore: adopt {SUBTREE_PREFIX} as git subtree from {WGPU_REMOTE}/{DEFAULT_BRANCH}",
        )
    else:
        print(f"\n'{SUBTREE_PREFIX}' does not exist. Adding subtree…")
        git(
            "subtree", "add",
            "--prefix", SUBTREE_PREFIX,
            WGPU_REMOTE, DEFAULT_BRANCH,
            "--squash",
            "-m", f"chore: add {SUBTREE_PREFIX} as git subtree from {WGPU_REMOTE}/{DEFAULT_BRANCH}",
        )

    print("\n✓ Subtree setup complete.")


def cmd_pull() -> None:
    """Pull upstream wgpu changes (squash)."""
    print("\n=== sync_wgpu pull ===\n")
    ensure_clean_worktree()

    if not remote_exists(WGPU_REMOTE):
        abort(f"Remote '{WGPU_REMOTE}' not found. Run 'setup' first.")

    git("fetch", WGPU_REMOTE)
    git(
        "subtree", "pull",
        "--prefix", SUBTREE_PREFIX,
        WGPU_REMOTE, DEFAULT_BRANCH,
        "--squash",
        "-m", f"chore: pull wgpu updates from {WGPU_REMOTE}/{DEFAULT_BRANCH}",
    )
    print("\n✓ Subtree pull complete.")


def cmd_push() -> None:
    """Push local wgpu-hal patches to the fork."""
    print("\n=== sync_wgpu push ===\n")
    ensure_clean_worktree()

    if not remote_exists(WGPU_REMOTE):
        abort(f"Remote '{WGPU_REMOTE}' not found. Run 'setup' first.")

    git(
        "subtree", "push",
        "--prefix", SUBTREE_PREFIX,
        WGPU_REMOTE, DEFAULT_BRANCH,
    )
    print("\n✓ Subtree push complete.")


# ── CLI ──────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Git Subtree helper for third_party/wgpu in GLASS.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("setup", help="Configure remote + initial subtree add/adopt")
    sub.add_parser("pull", help="Pull upstream wgpu changes (--squash)")
    sub.add_parser("push", help="Push local wgpu-hal patches to fork")
    sub.add_parser("status", help="Show current remote and subtree state")

    args = parser.parse_args()

    # Ensure we're at repo root
    import os
    os.chdir(repo_root())

    commands = {
        "setup": cmd_setup,
        "pull": cmd_pull,
        "push": cmd_push,
        "status": cmd_status,
    }
    commands[args.command]()


if __name__ == "__main__":
    main()
