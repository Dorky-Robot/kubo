#!/bin/bash
# kubo container entrypoint — configure git from env vars

# Set git identity if provided via env
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

exec "$@"
