## Project

`termhelp` — a terminal helper that turns a terminal command into a prompt which generates a terminal command (per `README.md`).

## Current state

The repository is a fresh scaffold: only `LICENSE` and `README.md` are present. There is no source code, build system, dependency manifest, or test suite yet. When starting work here, expect to pick the language/toolchain and create the initial project layout — confirm the choice with the user before scaffolding.

Once a toolchain is in place, update this file with the actual build/test/lint commands and the architecture of the helper (input parsing, LLM/prompt layer, command-generation output).

## Interpreter

This project calls LLMs through the use of the rust binary, interpreter. The interpreter accepts the current shell command line and then extracts additional information from the shell such as which shell it is, bash, zsh, fish, etc., and then the history of the shell. The interpreter passes this information to an LLM which returns a command to replace the input to the interpreter in the shell line. This command should be executable by the user and should properly execute a shell command to produce the output the user desires.
