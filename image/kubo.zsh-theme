# kubo — two-line prompt, plain ASCII
# twilight palette: dusty lavender (103), warm sand (143), muted sage (108),
#                   soft rose (131), quiet gray (245), deep mute (59)

setopt prompt_subst
autoload -Uz vcs_info

zstyle ':vcs_info:*' enable git
zstyle ':vcs_info:*' check-for-changes true
zstyle ':vcs_info:*' unstagedstr '%F{131}*%f'
zstyle ':vcs_info:*' stagedstr '%F{143}+%f'
zstyle ':vcs_info:git:*' formats ' %F{103}%b%f%u%c'
zstyle ':vcs_info:git:*' actionformats ' %F{103}%b%f %F{131}%a%f%u%c'

precmd() {
  vcs_info
  print -Pn "\e]0;kubo:${KUBO_NAME:-kubo} %~\a"
}

# two-line prompt:
#   kubo:name  path branch*
#   >
PROMPT='%F{59}kubo:${KUBO_NAME:-kubo}%f  %F{103}%~%f${vcs_info_msg_0_}
%F{245}>%f '
RPROMPT='%(?..%F{131}%?%f)'
