# termhelp / interpreter — zsh integration
#
# Source this file from ~/.zshrc:
#
#     source /path/to/termhelp/shell/interpreter.zsh
#
# Press Ctrl-G on the command line to send the current buffer to
# `interpreter` and replace it in-place with the generated command.
# Rebind by exporting INTERPRETER_KEY (zsh escape syntax, e.g. '^G', '^[i').

: ${INTERPRETER_BIN:=interpreter}
: ${INTERPRETER_KEY:=^G}
: ${INTERPRETER_CONFIG:=$HOME/.config/interpreter/config.json}

_interpreter_widget() {
  emulate -L zsh
  local input=$BUFFER
  local cursor=$CURSOR

  # Show a hint while we wait; zle -R repaints the prompt area.
  zle -R "interpreter: thinking…"

  local output
  if ! output=$("$INTERPRETER_BIN" --config "$INTERPRETER_CONFIG" -- "$input" 2>/dev/null); then
    zle -M "interpreter: failed (exit $?)"
    return 1
  fi

  # Trim trailing newline; the binary prints exactly one line.
  output=${output%$'\n'}
  [[ -z $output ]] && { zle -M "interpreter: empty response"; return 1 }

  BUFFER=$output
  CURSOR=${#BUFFER}
  zle -R
}

zle -N _interpreter_widget
bindkey "$INTERPRETER_KEY" _interpreter_widget
