# terminal-interpreter / interpreter — bash integration
#
# Source this file from ~/.bashrc:
#
#     source /path/to/terminal-interpreter/shell/interpreter.bash
#
# Press Ctrl-G on the command line to send the current readline buffer to
# `interpreter` and replace it in-place. Rebind by exporting
# INTERPRETER_KEY in readline syntax (e.g. '"\C-g"', '"\e i"').

: "${INTERPRETER_BIN:=interpreter}"
: "${INTERPRETER_KEY:=\"\\C-g\"}"
if [[ -z ${INTERPRETER_CONFIG+x} ]]; then
  case "$(uname -s)" in
    Darwin) INTERPRETER_CONFIG="$HOME/Library/Application Support/interpreter/config.json" ;;
    *)      INTERPRETER_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/interpreter/config.json" ;;
  esac
fi

_interpreter_widget() {
  local input=$READLINE_LINE
  local output
  if ! output=$("$INTERPRETER_BIN" --config "$INTERPRETER_CONFIG" -- "$input" 2>/dev/null); then
    return 1
  fi
  output=${output%$'\n'}
  [[ -z $output ]] && return 1

  READLINE_LINE=$output
  READLINE_POINT=${#READLINE_LINE}
}

# `bind -x` runs the function with READLINE_{LINE,POINT} exposed and writes
# them back to the buffer on return.
eval "bind -x '${INTERPRETER_KEY}: _interpreter_widget'"
