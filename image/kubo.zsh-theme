# kubo — flat twilight prompt with nerd font container icon
# twilight palette: dusty lavender (103), warm sand (143), muted sage (108),
#                   soft rose (131), quiet gray (245), deep mute (59)

setopt prompt_subst
autoload -Uz vcs_info

zstyle ':vcs_info:*' enable git
zstyle ':vcs_info:*' check-for-changes true
zstyle ':vcs_info:*' unstagedstr '%F{131}*%f'
zstyle ':vcs_info:*' stagedstr '%F{143}+%f'
zstyle ':vcs_info:git:*' formats ' %F{103} %b%f%u%c'
zstyle ':vcs_info:git:*' actionformats ' %F{103} %b%f %F{131}%a%f%u%c'

# ── kubo detection ──────────────────────────────────────────────────
# Nerd Font container icon when running inside kubo
if [[ -f /usr/local/bin/kubo-entrypoint ]]; then
  _KUBO_TAG='%F{59}󰡨%f '
else
  _KUBO_TAG=''
fi

precmd() {
  vcs_info
  # OSC title — works over SSH, ignored by terminals that don't support it
  print -Pn "\e]0;${_KUBO_TAG:+kubo ∙ }%~\a"
}

# flat prompt: 󰡨 dir  branch ›
PROMPT='${_KUBO_TAG}%F{103}%2~%f${vcs_info_msg_0_} %F{245}›%f '
RPROMPT='%(?..%F{131}%?%f)'
