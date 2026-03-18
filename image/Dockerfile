FROM debian:bookworm-slim

ARG TARGETARCH

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8
ENV TERM=xterm-256color

# ── Locale (set up first to avoid perl warnings during later apt installs) ─
RUN PERL_BADLANG=0 apt-get update && PERL_BADLANG=0 apt-get install -y --no-install-recommends locales \
    && sed -i '/en_US.UTF-8/s/^# //' /etc/locale.gen && locale-gen \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# ── System packages (minimal) ─────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
    # build essentials
    build-essential pkg-config \
    ca-certificates curl wget git openssh-client \
    # terminal
    zsh tmux jq htop less \
    # libs for rust crates
    libssl-dev libsqlite3-dev \
    # clipboard (for katulong image paste → Claude Code via xclip + Xvfb)
    xclip xvfb \
    # misc
    unzip file sudo procps \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# ── Modern CLI tools (pinned versions) ────────────────────────────
# aarch64 uses gnu (no musl builds available), x86_64 uses musl
RUN ARCH=$(uname -m) \
    && LIBC=$([ "$ARCH" = "aarch64" ] && echo "gnu" || echo "musl") \
    && FZF_ARCH=$(echo $ARCH | sed 's/aarch64/arm64/' | sed 's/x86_64/amd64/') \
    # ripgrep
    && curl -fsSL "https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-${ARCH}-unknown-linux-${LIBC}.tar.gz" \
    | tar -xz -C /tmp && cp /tmp/ripgrep-*/rg /usr/local/bin/ && rm -rf /tmp/ripgrep-* \
    # fd (has musl for both archs)
    && curl -fsSL "https://github.com/sharkdp/fd/releases/download/v10.4.2/fd-v10.4.2-${ARCH}-unknown-linux-${LIBC}.tar.gz" \
    | tar -xz -C /tmp && cp /tmp/fd-*/fd /usr/local/bin/ && rm -rf /tmp/fd-* \
    # bat (has musl for both archs)
    && curl -fsSL "https://github.com/sharkdp/bat/releases/download/v0.26.1/bat-v0.26.1-${ARCH}-unknown-linux-${LIBC}.tar.gz" \
    | tar -xz -C /tmp && cp /tmp/bat-*/bat /usr/local/bin/ && rm -rf /tmp/bat-* \
    # eza
    && curl -fsSL "https://github.com/eza-community/eza/releases/download/v0.23.4/eza_${ARCH}-unknown-linux-${LIBC}.tar.gz" \
    | tar -xz -C /tmp && cp /tmp/eza /usr/local/bin/ && rm -rf /tmp/eza \
    # fzf
    && curl -fsSL "https://github.com/junegunn/fzf/releases/download/v0.70.0/fzf-0.70.0-linux_${FZF_ARCH}.tar.gz" \
    | tar -xz -C /usr/local/bin/ \
    # delta
    && curl -fsSL "https://github.com/dandavison/delta/releases/download/0.18.2/delta-0.18.2-${ARCH}-unknown-linux-${LIBC}.tar.gz" \
    | tar -xz -C /tmp && cp /tmp/delta-*/delta /usr/local/bin/ && rm -rf /tmp/delta-*


# ── Rust (via rustup) ─────────────────────────────────────────────
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH="/usr/local/cargo/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --profile minimal \
    && rustup component add clippy rustfmt

# ── sipag (PR automation CLI) ──────────────────────────────────────
RUN curl -fsSL https://raw.githubusercontent.com/Dorky-Robot/sipag/main/scripts/install.sh | sh

# ── Node.js (via fnm) ────────────────────────────────────────────
ENV FNM_DIR=/usr/local/fnm
ENV PATH="/usr/local/fnm/aliases/default/bin:${PATH}"

ENV NPM_CONFIG_UPDATE_NOTIFIER=false

RUN curl -fsSL https://fnm.vercel.app/install | bash -s -- --install-dir /usr/local/fnm --skip-shell \
    && /usr/local/fnm/fnm install 22 \
    && /usr/local/fnm/fnm default 22 \
    && /usr/local/fnm/fnm alias default 22 \
    && npm install -g https://github.com/Dorky-Robot/yolo.git

# ── Katulong (web terminal) ──────────────────────────────────────
ENV HUSKY=0
RUN curl -fsSL https://raw.githubusercontent.com/dorky-robot/katulong/main/install.sh | sh

# ── Go (latest stable) ───────────────────────────────────────────
ENV PATH="/usr/local/go/bin:${PATH}"

RUN GO_V=$(curl -fsSL 'https://go.dev/dl/?mode=json' | jq -r '.[0].version') \
    && GOARCH=$(uname -m | sed 's/aarch64/arm64/' | sed 's/x86_64/amd64/') \
    && curl -fsSL "https://go.dev/dl/${GO_V}.linux-${GOARCH}.tar.gz" | tar -C /usr/local -xz

# ── Cloudflared (for tunnels / staging) ───────────────────────────
RUN CF_ARCH=$(uname -m | sed 's/aarch64/arm64/' | sed 's/x86_64/amd64/') \
    && curl -fsSL "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${CF_ARCH}" -o /usr/local/bin/cloudflared \
    && chmod +x /usr/local/bin/cloudflared

# ── GitHub CLI ────────────────────────────────────────────────────
RUN GH_ARCH=$(uname -m | sed 's/aarch64/arm64/' | sed 's/x86_64/amd64/') \
    && curl -fsSL "https://github.com/cli/cli/releases/download/v2.88.1/gh_2.88.1_linux_${GH_ARCH}.tar.gz" \
    | tar -xz -C /tmp && cp /tmp/gh_*/bin/gh /usr/local/bin/ && rm -rf /tmp/gh_*

# ── Dev user (non-root with sudo) ────────────────────────────────
RUN useradd -m -s /usr/bin/zsh -G sudo dev \
    && echo 'dev ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers.d/dev

USER dev
WORKDIR /home/dev

# ── Claude Code (install as dev user) ────────────────────────────
RUN curl -fsSL https://claude.ai/install.sh | bash
ENV PATH="/home/dev/.local/bin:${PATH}"

# ── Oh My Zsh ────────────────────────────────────────────────────
RUN sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended \
    && git clone --depth 1 https://github.com/zsh-users/zsh-autosuggestions ${ZSH_CUSTOM:-~/.oh-my-zsh/custom}/plugins/zsh-autosuggestions \
    && git clone --depth 1 https://github.com/zsh-users/zsh-syntax-highlighting ${ZSH_CUSTOM:-~/.oh-my-zsh/custom}/plugins/zsh-syntax-highlighting

# ── Shell & editor config ────────────────────────────────────────
COPY --chown=dev:dev zshrc /home/dev/.zshrc
COPY --chown=dev:dev kubo.zsh-theme /home/dev/.oh-my-zsh/custom/themes/kubo.zsh-theme
COPY --chown=dev:dev tmux.conf /home/dev/.tmux.conf
COPY --chown=dev:dev vimrc /home/dev/.vimrc

# ── Clipboard bridge (for katulong image paste) ──────────────────
# DISPLAY must be set via ENV (not just entrypoint export) so that
# docker exec sessions inherit it. Without this, Claude Code inside
# a `kubo` shell can't read the X clipboard via xclip — the entrypoint
# starts Xvfb on :99 but `export DISPLAY=:99` only affects PID 1's
# children, not processes spawned by docker exec.
ENV DISPLAY=:99
COPY pbpaste pbcopy clip /usr/local/bin/
USER root
RUN chmod +x /usr/local/bin/pbpaste /usr/local/bin/pbcopy /usr/local/bin/clip

# ── Welcome banner ──────────────────────────────────────────────
RUN mkdir -p /usr/local/lib/kubo
COPY welcome.sh /usr/local/lib/kubo/welcome.sh

# ── Entrypoint (configures git from env vars) ────────────────────
COPY entrypoint.sh /usr/local/bin/kubo-entrypoint
RUN chmod +x /usr/local/bin/kubo-entrypoint
USER dev

# ── Snapshot home directory defaults for volume initialization ────
# When /home/dev is a named volume, the entrypoint copies these defaults
# on first run and refreshes system files on upgrade.
USER root
RUN cp -a /home/dev /etc/skel.kubo
USER dev

WORKDIR /work
ENTRYPOINT ["kubo-entrypoint"]
CMD ["sleep", "infinity"]
