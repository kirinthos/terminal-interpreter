# terminal-interpreter

> Turn a half-remembered intent at your shell prompt into the command you
> actually wanted to type.

`terminal-interpreter` is for developers who keep stopping at the prompt to think "how do I
list files modified in the last 24 hours that contain *foo*?" — and would
rather just write that sentence and hit a key. The `interpreter` binary takes
the current command line plus a snapshot of your shell environment (which
shell you're in, your cwd, your recent history) and asks an LLM to rewrite
your input into a single executable command. A shell widget pipes the current
line through the binary and substitutes the result back in place so you can
review and run it.

It's intentionally small: one binary, one config file, a thin shell wrapper.

---

## Features

- **In-place command rewriting.** Press a hotkey (default `Ctrl-G`); your
  prompt is replaced with a real, runnable command you can edit before
  pressing Enter.
- **Multiple providers.** First-class support for OpenAI, Anthropic, and a
  local Ollama server. Switch via a single `provider/model-name` string.
- **Live model catalogue with pricing.** `--model-list` pulls the current
  model list and per-token pricing from the [LiteLLM][litellm] catalogue, so
  the picker stays current as providers release new models.
- **Built-in configuration TUI.** `--init` opens a navigable menu (arrow keys
  *or* vim bindings) where every config field has a description; the `model`
  field opens a searchable picker (`/` to filter) populated from the live
  catalogue.
- **Shell-aware prompts.** The user prompt sent to the model includes the
  shell kind (bash / zsh / fish), the current OS, the working directory, and
  the tail of your shell history — read fresh on every invocation so commands
  you just ran are part of the context.
- **No daemon, no background process.** A single CLI invocation per
  rewrite. The shell widget calls it, gets a line back on stdout, replaces
  the buffer.

[litellm]: https://github.com/BerriAI/litellm

---

## How it works

```
   ┌──────────────────────────┐
   │  you type at the prompt  │
   │  "list files w/o tests"  │
   └────────────┬─────────────┘
                │ Ctrl-G
                ▼
   ┌──────────────────────────┐         ┌────────────────────┐
   │  shell widget captures   │ stdin   │  interpreter       │
   │  $BUFFER, calls          ├────────►│  ─ load config     │
   │  `interpreter --config…` │         │  ─ read history    │
   └──────────────────────────┘         │  ─ build prompt    │
                                        │  ─ call LLM        │
                                        │  ─ sanitize fences │
                                        └─────────┬──────────┘
                ┌─────────────────────────────────┘
                ▼
   ┌──────────────────────────┐
   │  $BUFFER ← stdout        │
   │  cursor at end of line   │
   │  press Enter to run      │
   └──────────────────────────┘
```

---

## Installation

### From source

You need a Rust toolchain (edition 2024, currently building on stable). The
repo ships a Cargo workspace with one member, the `interpreter` crate.

```sh
git clone https://github.com/kirinthos/terminal-interpreter.git
cd terminal-interpreter
cargo build --release -p interpreter
# binary lives at target/release/interpreter
```

Drop the binary somewhere on your `PATH`, or symlink it:

```sh
sudo install -m 0755 target/release/interpreter /usr/local/bin/interpreter
```

### One-shot setup with `--install`

The fastest path from zero to working setup:

```sh
interpreter --install
```

This will:

1. Detect your current shell from `$SHELL` (bash / zsh / fish).
2. Write the appropriate `interpreter.<shell>` integration script into your
   config directory (`~/.config/interpreter/` by default).
3. Open the configuration TUI so you can pick a model, set provider keys,
   tune `history_limit`, etc.
4. After you save and quit, print the exact `echo 'source …' >> ~/.<rc>`
   command you need to add to your shell's rc file, plus a usage hint.

After the rc file has been updated, open a new shell (or `source` your rc
file) and you're ready.

### Manual setup

If you'd rather wire it up yourself:

1. **Build the binary** (see [From source](#from-source)).
2. **Create a config file** at `~/.config/interpreter/config.json`. Minimal
   example:
   ```json
   {
     "model": "anthropic/claude-haiku-4-5",
     "history_limit": 50,
     "temperature": 0.0,
     "providers": {
       "anthropic": { "api_key": "sk-ant-…" }
     }
   }
   ```
   Or run `interpreter --init` to do the same through the TUI.
3. **Source the right shell file** in your rc. Pick the file under
   `shell/` (or copy it into `~/.config/interpreter/`):
   ```sh
   # ~/.zshrc
   source ~/.config/interpreter/interpreter.zsh

   # ~/.bashrc
   source ~/.config/interpreter/interpreter.bash

   # ~/.config/fish/config.fish
   source ~/.config/interpreter/interpreter.fish
   ```
4. **Open a new shell**. Type a prompt, press `Ctrl-G`.

---

## Usage

At your shell prompt, type what you want in plain English (or a partial
command), then press the hotkey:

```
$ list files including hidden, sorted by mtime ^G
$ ls -alt
```

The buffer is rewritten in place. Edit if you need to, press Enter to run.

The binary itself is also runnable directly:

```sh
interpreter "show me processes hogging memory"
# → ps aux --sort=-%mem | head
```

### CLI flags

```
interpreter [OPTIONS] [COMMAND]...

  -m, --model <PROVIDER/MODEL>   Override the configured model for this call
                                 (e.g. anthropic/claude-opus-4-7).
  -c, --config <PATH>            Config file path. Also reads
                                 $INTERPRETER_CONFIG; otherwise falls back to
                                 $XDG_CONFIG_HOME/interpreter/config.json.
      --dry-run                  Print the resolved config and exit.
      --model-list               Print every known model from each configured
                                 provider, with pricing, in the exact
                                 `provider/model-name` form the config uses.
      --init                     Open the configuration TUI.
      --install                  Install the shell integration for the current
                                 shell, open the TUI, then print rc-file setup
                                 instructions.
  -h, --help                     Print help.
  -V, --version                  Print version.
```

### Environment variables

| Variable             | Purpose                                                                                                  |
| -------------------- | -------------------------------------------------------------------------------------------------------- |
| `INTERPRETER_CONFIG` | Path to the config file. Overridden by `--config`.                                                       |
| `INTERPRETER_BIN`    | Read by the shell integration scripts to locate the binary. Defaults to `interpreter` (uses `$PATH`).    |
| `INTERPRETER_KEY`    | Override the hotkey. Shell-specific syntax — e.g. `^G` in zsh, `"\C-g"` in bash, `\cg` in fish.          |

---

## Configuration

The config file is JSON, by default at
`~/.config/interpreter/config.json`. Schema:

| Key             | Type             | Default                  | Description                                                                       |
| --------------- | ---------------- | ------------------------ | --------------------------------------------------------------------------------- |
| `model`         | `string`         | `"openai/gpt-4o-mini"`   | Default model in `provider/model-name` form.                                       |
| `history_limit` | `usize`          | `50`                     | How many recent shell-history lines to read off the history file and send to the LLM. |
| `temperature`   | `float \| null`  | `null`                   | Sampling temperature. Leave null to use the provider default.                     |
| `system_prompt` | `string \| null` | `null`                   | Override the built-in system prompt.                                              |
| `providers`     | `object`         | `{}`                     | Per-provider settings — see below.                                                |

Each provider entry under `providers.<name>` can carry:

| Key        | Type             | Notes                                                                  |
| ---------- | ---------------- | ---------------------------------------------------------------------- |
| `api_key`  | `string \| null` | Required for OpenAI and Anthropic. Not needed for Ollama.              |
| `base_url` | `string \| null` | Reserved for future support; not yet plumbed through the LLM builder.  |

Unknown top-level fields are rejected — typos surface as errors instead of
being silently ignored.

### Picking a model

To list every model the binary knows about, with current pricing:

```sh
interpreter --model-list
```

```
anthropic/claude-haiku-4-5       $   1.00 in / $   5.00 out per 1M tok
anthropic/claude-opus-4-7        $  15.00 in / $  75.00 out per 1M tok
ollama/llama3.2:latest           (local)
openai/gpt-4o-mini               $   0.15 in / $   0.60 out per 1M tok
openai/o1-mini                   $   3.00 in / $  12.00 out per 1M tok
...
```

Pricing is pulled live from the [LiteLLM model catalogue][litellm] each time
the flag is run, so it tracks provider changes without requiring a release
of this tool.

The `--init` TUI exposes the same list under the `model` menu entry, with `/`
to filter (contains-match, case-insensitive). Press Enter to confirm the
filter, then pick a model.

---

## Provider support

### OpenAI

```json
{
  "model": "openai/gpt-4o-mini",
  "providers": {
    "openai": { "api_key": "sk-..." }
  }
}
```

### Anthropic

```json
{
  "model": "anthropic/claude-haiku-4-5",
  "providers": {
    "anthropic": { "api_key": "sk-ant-..." }
  }
}
```

### Ollama (local)

If you'd rather keep prompts on-device, point `interpreter` at a local
[Ollama][ollama] server. There's nothing to configure beyond installing
Ollama, pulling a model, and pointing the config at it:

```sh
# install Ollama, then:
ollama pull llama3.2
```

```json
{
  "model": "ollama/llama3.2",
  "providers": {
    "ollama": {}
  }
}
```

`--model-list` will autodetect a running Ollama server on
`http://localhost:11434` and include its tags in the output (no API key
required). If the server isn't running, Ollama entries are simply omitted.

[ollama]: https://ollama.com

---

## Repository layout

```
.
├── Cargo.toml                 # workspace
├── interpreter/               # the binary + library crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs            # entrypoint, flag dispatch
│       ├── lib.rs             # module roots
│       ├── cli.rs             # clap definitions
│       ├── config.rs          # JSON config schema + load/save
│       ├── shell.rs           # shell detection + tail-of-history reader
│       ├── llm_client.rs      # prompt builder + `llm` crate routing
│       ├── model_list.rs      # --model-list (LiteLLM + Ollama)
│       ├── init_tui.rs        # --init configuration TUI (cursive)
│       ├── install.rs         # --install (shell integration + TUI)
│       └── prompts/
│           └── default_system.md
├── shell/                     # shell integration scripts
│   ├── interpreter.zsh
│   ├── interpreter.bash
│   └── interpreter.fish
├── .envrc                     # direnv: points $INTERPRETER_BIN at debug build
├── dev-config.json            # local dev config (gitignored)
└── README.md
```

---

## Development

The repo is a Cargo workspace; the only member is `interpreter`. There's a
direnv `.envrc` that wires `INTERPRETER_BIN` to `target/debug/interpreter` so
the shell integration uses the working copy when you're in the repo.

```sh
# build + run tests
cargo build -p interpreter
cargo test  -p interpreter --lib

# live model-list eval harness (network, opt-in)
cargo test  -p interpreter --test eval -- --ignored
```

A couple of things worth knowing if you're extending this:

- The `llm` crate (v1.x) drives all provider routing in `llm_client.rs`. It
  exposes a builder API; everything goes through `LLMBuilder::new().backend(…)`.
- Model-catalogue pricing comes from a single upstream JSON file — see
  `PRICING_URL` in `model_list.rs`. Schema may drift; the deserialiser is
  intentionally tolerant.
- The configuration TUI is a thin layer over the `cursive` crate. Widgets are
  built once per screen; the shared `State` (`Arc<Mutex<…>>`) is passed by
  clone into every callback because cursive callbacks must be
  `Send + Sync + 'static`.
- Shell history reading is a backwards tail-seek on the history file (no
  full-file read), bounded by the requested line count. Zsh's
  `: <ts>:<dur>;cmd` lines are stripped to just `cmd`.

---

## Security notes

- **Your prompts and history go to the model provider.** Whatever the LLM sees,
  the provider sees. If you don't want commercial providers to see your
  shell context, use Ollama.
- **The output is a shell command, not pre-executed.** The widget replaces
  your buffer but doesn't press Enter for you; the run is always an explicit
  action.
- **Keys live in plaintext in the config file.** Treat that file like any
  other secret-bearing artifact: it shouldn't be checked in, world-readable,
  or sitting in a synced backup target unless you've thought about it.
- **Bash's `bind -x` is line-mode only.** Don't expect the integration to
  capture multi-line buffers or vi-mode-edit mid-command.

---

## License

MIT. See [LICENSE](./LICENSE).
