#!/bin/zsh
# kubo welcome banner — shown once per session via KUBO_WELCOMED guard

_kubo_welcome() {
  local C='\033[38;5;103m'   # dusty lavender
  local D='\033[38;5;245m'   # quiet gray
  local S='\033[38;5;143m'   # warm sand
  local E='\033[38;5;131m'   # soft rose
  local R='\033[0m'          # reset

  local name="${KUBO_NAME:-kubo}"

  print ""
  print "  ${C}kubo:${name}${R}"
  print ""

  # ── Quick reference ────────────────────────────────────────────────
  print "  ${S}yolo${R}              ${D}claude --dangerously-skip-permissions${R}"
  print "  ${S}yolo --resume${R}     ${D}resume last claude session${R}"
  print "  ${S}claude${R}            ${D}claude code (normal mode)${R}"
  print ""

  # ── Tools ──────────────────────────────────────────────────────────
  local tools=""
  command -v rustc      &>/dev/null && tools+="rust "
  command -v node       &>/dev/null && tools+="node "
  command -v go         &>/dev/null && tools+="go "
  command -v gh         &>/dev/null && tools+="gh "
  command -v cloudflared &>/dev/null && tools+="cloudflared "
  command -v katulong   &>/dev/null && tools+="katulong "
  command -v sipag      &>/dev/null && tools+="sipag "

  [[ -n "$tools" ]] && print "  ${D}tools${R}  ${D}${tools}${R}"

  local cli=""
  command -v rg    &>/dev/null && cli+="rg "
  command -v fd    &>/dev/null && cli+="fd "
  command -v bat   &>/dev/null && cli+="bat "
  command -v eza   &>/dev/null && cli+="eza "
  command -v fzf   &>/dev/null && cli+="fzf "
  command -v delta &>/dev/null && cli+="delta "

  [[ -n "$cli" ]] && print "  ${D}cli  ${R}  ${D}${cli}${R}"
  print ""

  # ── Tmux ───────────────────────────────────────────────────────────
  print "  ${D}tmux${R}   ${S}C-a${R}        ${D}prefix${R}"
  print "         ${S}C-a |${R}      ${D}split vertical${R}"
  print "         ${S}C-a -${R}      ${D}split horizontal${R}"
  print "         ${S}C-a d${R}      ${D}detach (session persists)${R}"
  print ""

  # ── Help ───────────────────────────────────────────────────────────
  print "  ${D}On the host:${R}"
  print "  ${S}kubo ls${R}            ${D}list containers${R}"
  print "  ${S}kubo update ${name}${R}  ${D}rebuild image + recreate (keeps data)${R}"
  print "  ${S}kubo restart ${name}${R} ${D}restart container${R}"
  print "  ${S}kubo add ${name} dir${R} ${D}mount another directory${R}"
  print ""
}

_kubo_welcome
unfunction _kubo_welcome
