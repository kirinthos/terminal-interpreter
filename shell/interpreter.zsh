# terminal-interpreter / interpreter — zsh integration
#
# Source this file from ~/.zshrc:
#
#     source /path/to/terminal-interpreter/shell/interpreter.zsh
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

  # `zle -R msg` shows a transient status below the prompt.
  zle -R "interpreter: thinking…"

  local output rc
  output=$("$INTERPRETER_BIN" --config "$INTERPRETER_CONFIG" -- "$input" 2>/dev/null)
  rc=$?

  # Always force a full redraw before mutating BUFFER or printing messages,
  # otherwise the "thinking…" status line bleeds into the new prompt.
  if (( rc != 0 )); then
    zle reset-prompt
    zle -M "interpreter: failed (exit $rc)"
    return 1
  fi

  # Trim trailing newline; the binary prints exactly one line.
  output=${output%$'\n'}
  if [[ -z $output ]]; then
    zle reset-prompt
    zle -M "interpreter: empty response"
    return 1
  fi

  BUFFER=$output
  CURSOR=${#BUFFER}
  zle reset-prompt
}

zle -N _interpreter_widget
bindkey "$INTERPRETER_KEY" _interpreter_widget
