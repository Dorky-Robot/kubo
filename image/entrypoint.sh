#!/bin/bash
# kubo container entrypoint — initialize persistent home, configure git

SKEL=/etc/skel.kubo

# ── Persistent home volume initialization ────────────────────────
# When /home/dev is backed by a named volume, it starts empty on first
# container creation. We populate it from the image snapshot. On upgrades
# (new image, existing volume), we refresh system-managed files while
# preserving user data like ~/.claude, cloned repos, shell history, etc.

if [ ! -f /home/dev/.kubo-initialized ]; then
    # First run — copy all defaults from the image skeleton
    if [ -d "$SKEL" ]; then
        cp -a "$SKEL"/. /home/dev/
    fi
    touch /home/dev/.kubo-initialized
elif [ -d "$SKEL" ]; then
    # Upgrade — refresh system-managed files only
    # These are files kubo controls that should track the image version.
    for f in .zshrc .oh-my-zsh .tmux.conf .vimrc; do
        if [ -e "$SKEL/$f" ]; then
            rm -rf "/home/dev/$f"
            cp -a "$SKEL/$f" "/home/dev/$f"
        fi
    done
    # Ensure new tools from the image are available
    if [ -d "$SKEL/.local/bin" ]; then
        mkdir -p /home/dev/.local/bin
        cp -n "$SKEL/.local/bin"/* /home/dev/.local/bin/ 2>/dev/null || true
    fi
fi

# ── Git configuration ────────────────────────────────────────────
if [ -n "$GIT_AUTHOR_NAME" ]; then
    git config --global user.name "$GIT_AUTHOR_NAME"
fi
if [ -n "$GIT_AUTHOR_EMAIL" ]; then
    git config --global user.email "$GIT_AUTHOR_EMAIL"
fi

# If an SSH signing key is mounted, configure git to use it
if [ -n "$KUBO_GIT_SIGNING_KEY" ] && [ -f "$KUBO_GIT_SIGNING_KEY" ]; then
    git config --global gpg.format ssh
    git config --global user.signingkey "$KUBO_GIT_SIGNING_KEY"
    git config --global commit.gpgsign true
    git config --global gpg.ssh.program /usr/bin/ssh-keygen
fi

# gh credential helper
if command -v gh &>/dev/null; then
    git config --global credential.https://github.com.helper '!/usr/bin/gh auth git-credential'
    git config --global credential.https://gist.github.com.helper '!/usr/bin/gh auth git-credential'
fi

git config --global push.autoSetupRemote true
git config --global core.editor "vi"

# ── Delta (pretty diffs) ─────────────────────────────────────────
if command -v delta &>/dev/null; then
    git config --global core.pager delta
    git config --global interactive.diffFilter 'delta --color-only'
    git config --global delta.navigate true
    git config --global delta.line-numbers true
    git config --global delta.syntax-theme none
    git config --global delta.file-style 'bold 103'
    git config --global delta.hunk-header-style 'omit'
    git config --global delta.minus-style '131'
    git config --global delta.plus-style '108'
    git config --global delta.line-numbers-minus-style '131'
    git config --global delta.line-numbers-plus-style '108'
    git config --global delta.line-numbers-zero-style '240'
    git config --global merge.conflictStyle zdiff3
fi

# ── Virtual X display for clipboard (xclip) ──────────────────────
# Katulong uses xclip to bridge images from remote devices to the
# container clipboard. Claude Code reads images via xclip.
if command -v Xvfb &>/dev/null && [ -z "$DISPLAY" ]; then
    Xvfb :99 -screen 0 1x1x8 -nolisten tcp &>/dev/null &
    export DISPLAY=:99
fi

exec "$@"
