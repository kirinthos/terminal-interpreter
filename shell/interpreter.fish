# termhelp / interpreter — fish integration
#
# Source this file from ~/.config/fish/config.fish (or drop it in
# ~/.config/fish/conf.d/ to autoload):
#
#     source /path/to/termhelp/shell/interpreter.fish
#
# Press Ctrl-G on the command line to send the current buffer to
# `interpreter` and replace it in-place. Rebind by setting
# INTERPRETER_KEY (fish bind syntax, e.g. \cg, \ei).

set -q INTERPRETER_BIN; or set -g INTERPRETER_BIN interpreter
set -q INTERPRETER_KEY; or set -g INTERPRETER_KEY \cg

function _interpreter_widget
    set -l input (commandline)
    set -l output ($INTERPRETER_BIN -- "$input" 2>/dev/null)
    or return 1
    test -z "$output"; and return 1

    commandline -r -- "$output"
    commandline -f end-of-line
end

bind $INTERPRETER_KEY _interpreter_widget
