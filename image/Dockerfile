FROM ubuntu:24.04

ARG TARGETARCH

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8
ENV TERM=xterm-256color

# ── Core system packages ────────────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
    # essentials
    build-essential pkg-config cmake \
    ca-certificates curl wget git openssh-client \
    # terminal life
    zsh tmux fzf ripgrep fd-find bat eza jq htop tree less \
    # libs commonly needed by rust crates
    libssl-dev libsqlite3-dev \
    # networking
    iproute2 dnsutils iputils-ping netcat-openbsd socat \
    # locale
    locales \
    # misc
    unzip file sudo \
    && locale-gen en_US.UTF-8 \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# ── Symlinks for ubuntu-named tools ────────────────────────────────
RUN ln -sf /usr/bin/batcat /usr/local/bin/bat \
    && ln -sf /usr/bin/fdfind /usr/local/bin/fd

# ── Rust (via rustup) ──────────────────────────────────────────────
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH="/usr/local/cargo/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --profile default \
    && rustup component add clippy rustfmt

# ── Node.js (via fnm for speed) ───────────────────────────────────
ENV FNM_DIR=/usr/local/fnm
ENV PATH="/usr/local/fnm/aliases/default/bin:${PATH}"

RUN curl -fsSL https://fnm.vercel.app/install | bash -s -- --install-dir /usr/local/fnm --skip-shell \
    && /usr/local/fnm/fnm install 22 \
    && /usr/local/fnm/fnm default 22 \
    && /usr/local/fnm/fnm alias default 22

# ── Go ─────────────────────────────────────────────────────────────
ENV PATH="/usr/local/go/bin:${PATH}"

RUN GOARCH=$([ "$TARGETARCH" = "arm64" ] && echo "arm64" || echo "amd64") \
    && curl -fsSL "https://go.dev/dl/go1.24.1.linux-${GOARCH}.tar.gz" | tar -C /usr/local -xz

# ── GitHub CLI ─────────────────────────────────────────────────────
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    | tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
    && apt-get update && apt-get install -y gh \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# ── Claude Code ────────────────────────────────────────────────────
RUN npm install -g @anthropic-ai/claude-code

# ── Dev user (non-root with sudo) ─────────────────────────────────
RUN useradd -m -s /usr/bin/zsh -G sudo dev \
    && echo 'dev ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers.d/dev

USER dev
WORKDIR /home/dev

# ── Oh My Zsh ─────────────────────────────────────────────────────
RUN sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended \
    && git clone https://github.com/zsh-users/zsh-autosuggestions ${ZSH_CUSTOM:-~/.oh-my-zsh/custom}/plugins/zsh-autosuggestions \
    && git clone https://github.com/zsh-users/zsh-syntax-highlighting ${ZSH_CUSTOM:-~/.oh-my-zsh/custom}/plugins/zsh-syntax-highlighting

# ── Zsh config ─────────────────────────────────────────────────────
COPY --chown=dev:dev zshrc /home/dev/.zshrc

# ── Entrypoint (configures git from env vars) ─────────────────────
COPY entrypoint.sh /usr/local/bin/kubo-entrypoint
USER root
RUN chmod +x /usr/local/bin/kubo-entrypoint
USER dev

WORKDIR /work
ENTRYPOINT ["kubo-entrypoint"]
CMD ["sleep", "infinity"]
