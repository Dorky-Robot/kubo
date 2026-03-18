#!/bin/zsh
# kubo welcome banner — shown once per session via KUBO_WELCOMED guard

_kubo_welcome() {
  local C='\033[38;5;103m'   # dusty lavender
  local D='\033[38;5;245m'   # quiet gray
  local S='\033[38;5;143m'   # warm sand
  local R='\033[0m'          # reset

  print ""
  print "  ${C}kubo${R}"
  print ""

  # ── Tool inventory ──────────────────────────────────────────────────
  local tools=""
  command -v rustc   &>/dev/null && tools+="${S}rust${R} "
  command -v node    &>/dev/null && tools+="${S}node${R} "
  command -v go      &>/dev/null && tools+="${S}go${R} "
  command -v claude  &>/dev/null && tools+="${S}claude${R} "

  local cli=""
  command -v rg      &>/dev/null && cli+="rg "
  command -v fd      &>/dev/null && cli+="fd "
  command -v bat     &>/dev/null && cli+="bat "
  command -v eza     &>/dev/null && cli+="eza "
  command -v fzf     &>/dev/null && cli+="fzf "
  command -v delta   &>/dev/null && cli+="delta "
  command -v gh      &>/dev/null && cli+="gh "

  [[ -n "$tools" ]] && print "  ${D}lang${R}  $tools"
  [[ -n "$cli"   ]] && print "  ${D}cli ${R}  ${D}${cli}${R}"
  print ""
  print "  ${D}tmux prefix${R}  ${S}C-a${R}    ${D}splits${R}  ${S}C-a |${R}  ${S}C-a -${R}"
  print ""
}

_kubo_welcome
unfunction _kubo_welcome
