# terminal-interpreter / interpreter — fish integration
#
# Source this file from ~/.config/fish/config.fish (or drop it in
# ~/.config/fish/conf.d/ to autoload):
#
#     source /path/to/terminal-interpreter/shell/interpreter.fish
#
# Press Ctrl-G on the command line to send the current buffer to
# `interpreter` and replace it in-place. Rebind by setting
# INTERPRETER_KEY (fish bind syntax, e.g. \cg, \ei).

set -q INTERPRETER_BIN; or set -g INTERPRETER_BIN interpreter
set -q INTERPRETER_KEY; or set -g INTERPRETER_KEY \cg
if not set -q INTERPRETER_CONFIG
  if test (uname -s) = Darwin
    set -g INTERPRETER_CONFIG "$HOME/Library/Application Support/interpreter/config.json"
  else
    set -g INTERPRETER_CONFIG "${XDG_CONFIG_HOME:-$HOME/.config}/interpreter/config.json"
  end
end

function _interpreter_widget
    set -l input (commandline)
    set -l output ($INTERPRETER_BIN --config "$INTERPRETER_CONFIG" -- "$input" 2>/dev/null)
    or return 1
    test -z "$output"; and return 1

    commandline -r -- "$output"
    commandline -f end-of-line
end

bind $INTERPRETER_KEY _interpreter_widget
