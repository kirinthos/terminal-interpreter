//! `--init` configuration TUI built on `cursive`.
//!
//! The main menu lists every config key with a short description. Selecting an
//! item navigates to a detail screen where the value can be edited. The
//! `model` key opens a searchable picker populated from `model_list`.
//!
//! Navigation: arrow keys or vim (`j`/`k` for down/up, `l`/Enter to descend,
//! `h`/Esc to go back, `q` to quit from the main screen).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use cursive::Cursive;
use cursive::event::{Event, Key};
use cursive::theme::{BorderStyle, Color, PaletteColor};
use cursive::traits::*;
use cursive::view::Nameable;
use cursive::views::{
    Dialog, EditView, LinearLayout, OnEventView, Panel, ResizedView, SelectView, TextArea, TextView,
};

use crate::config::{Config, ProviderEnv};
use crate::model_list::{self, Row};

const MODEL_LIST_NAME: &str = "model_list";
const MODEL_FILTER_NAME: &str = "model_filter";

pub fn run(path: PathBuf, config: Config) -> Result<()> {
    let state = Arc::new(Mutex::new(State { config, path }));
    let mut siv = cursive::default();

    let base_theme = siv.current_theme().clone();
    siv.set_theme(dark_theme(base_theme));

    show_main_menu(&mut siv, state);
    siv.run();
    Ok(())
}

struct State {
    config: Config,
    path: PathBuf,
}

// ---------------------------------------------------------------------------
// Theme — terminal-flavoured dark mode.
// ---------------------------------------------------------------------------
//
// Goal: feel like a normal dark terminal session. Backgrounds let the
// terminal's own colour come through where we can, foreground is a soft
// near-white (not full white, which fights with most terminal themes), and
// the selection highlight is a desaturated blue-grey block that's visible
// without grabbing attention. Borders are simple ASCII; shadows are off so
// dialogs don't paint dark blocks over the surrounding terminal.

fn dark_theme(mut theme: cursive::theme::Theme) -> cursive::theme::Theme {
    use PaletteColor::*;

    // Soft off-white text on a black canvas. `TerminalDefault` lets the user's
    // own terminal background show through where possible.
    let fg_primary = Color::Rgb(0xd0, 0xd0, 0xd0);
    let fg_dim = Color::Rgb(0x90, 0x90, 0x90);
    let fg_faint = Color::Rgb(0x60, 0x60, 0x60);
    let accent = Color::Rgb(0x7a, 0xa2, 0xc7); // muted steel-blue for titles
    let accent_dim = Color::Rgb(0x4d, 0x6a, 0x88);
    let highlight = Color::Rgb(0x33, 0x3b, 0x4d); // unintrusive selection bar
    let highlight_inactive = Color::Rgb(0x22, 0x26, 0x33);

    theme.palette[Background] = Color::TerminalDefault;
    theme.palette[Shadow] = Color::TerminalDefault;
    theme.palette[View] = Color::TerminalDefault;

    theme.palette[Primary] = fg_primary;
    theme.palette[Secondary] = fg_dim;
    theme.palette[Tertiary] = fg_faint;

    theme.palette[TitlePrimary] = accent;
    theme.palette[TitleSecondary] = accent_dim;

    theme.palette[Highlight] = highlight;
    theme.palette[HighlightInactive] = highlight_inactive;
    theme.palette[HighlightText] = fg_primary;

    theme.shadow = false;
    theme.borders = BorderStyle::Simple;
    theme
}

// ---------------------------------------------------------------------------
// Field schema — single source of truth for menu entries and descriptions.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
enum Field {
    Model,
    HistoryLimit,
    Temperature,
    SystemPrompt,
    AdditionalContext,
    ContextFiles,
    Plugins,
    OpenAi,
    Anthropic,
    Ollama,
    Thinking,
}

impl Field {
    fn label(&self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::HistoryLimit => "history_limit",
            Self::Temperature => "temperature",
            Self::SystemPrompt => "system_prompt",
            Self::AdditionalContext => "additional_context",
            Self::ContextFiles => "context_files",
            Self::Plugins => "plugins",
            Self::Thinking => "thinking",
            Self::OpenAi => "providers.openai",
            Self::Anthropic => "providers.anthropic",
            Self::Ollama => "providers.ollama",
        }
    }
    fn description(&self) -> &'static str {
        match self {
            Self::Model => {
                "Default model in `provider/model-name` form. Pick from a live catalogue."
            }
            Self::HistoryLimit => {
                "How many recent shell-history lines to read off the history file and send to the LLM."
            }
            Self::Temperature => {
                "Sampling temperature (0.0–2.0). Leave blank to use the provider default."
            }
            Self::SystemPrompt => {
                "Override the system prompt sent to the model. Leave blank for the default."
            }
            Self::AdditionalContext => {
                "Extra text appended to the system prompt on every request. Leave blank to omit."
            }
            Self::ContextFiles => {
                "File paths whose contents are injected as high-priority context into the system prompt."
            }
            Self::Plugins => {
                "Shell commands run before each request; their stdout is injected into the prompt."
            }
            Self::OpenAi => "OpenAI provider settings: api_key, base_url.",
            Self::Anthropic => "Anthropic provider settings: api_key, base_url.",
            Self::Ollama => "Ollama provider settings: api_key (usually unset), base_url.",
            Self::Thinking => "Enable model thinking output (e.g., CoT traces). Toggle on/off.",
        }
    }
}

const FIELDS: &[Field] = &[
    Field::Model,
    Field::HistoryLimit,
    Field::Temperature,
    Field::SystemPrompt,
    Field::AdditionalContext,
    Field::ContextFiles,
    Field::Plugins,
    Field::OpenAi,
    Field::Anthropic,
    Field::Ollama,
    Field::Thinking,
];

// ---------------------------------------------------------------------------
// Main menu
// ---------------------------------------------------------------------------

fn show_main_menu(siv: &mut Cursive, state: Arc<Mutex<State>>) {
    let thinking = state.lock().unwrap().config.thinking;
    let mut select: SelectView<Field> = SelectView::new();
    let label_w = FIELDS.iter().map(|f| f.label().len()).max().unwrap_or(0);
    for f in FIELDS {
        let desc = match f {
            Field::Thinking => {
                let mark = if thinking { "[x]" } else { "[ ]" };
                format!("{mark}  {}", f.description())
            }
            _ => f.description().to_string(),
        };
        let label = format!("{:<w$}  {}", f.label(), desc, w = label_w);
        select.add_item(label, *f);
    }
    select.set_on_submit({
        let state = state.clone();
        move |s, f: &Field| handle_field(s, state.clone(), *f)
    });
    let select = vim_keys(select.with_name("main_menu"));

    let dialog = Dialog::around(select.scrollable().full_width().min_height(12))
        .title("interpreter config")
        .button("Save", {
            let state = state.clone();
            move |s| save_and_notify(s, &state)
        })
        .button("Quit", |s| s.quit());
    siv.pop_layer();
    siv.add_layer(OnEventView::new(dialog).on_event('q', |s| s.quit()));
}

fn handle_field(siv: &mut Cursive, state: Arc<Mutex<State>>, field: Field) {
    match field {
        Field::Model => show_model_picker(siv, state),
        Field::HistoryLimit => edit_usize(
            siv,
            state.clone(),
            field,
            state.lock().unwrap().config.history_limit,
            |c, v| c.history_limit = v,
        ),
        Field::Temperature => edit_optional_f32(
            siv,
            state.clone(),
            field,
            state.lock().unwrap().config.temperature,
            |c, v| c.temperature = v,
        ),
        Field::SystemPrompt => edit_optional_string(
            siv,
            state.clone(),
            field,
            state.lock().unwrap().config.system_prompt.clone(),
            |c, v| c.system_prompt = v,
        ),
        Field::AdditionalContext => edit_multiline_string(
            siv,
            state.clone(),
            field,
            state.lock().unwrap().config.additional_context.clone(),
            |c, v| c.additional_context = v,
        ),
        Field::ContextFiles => edit_context_files(siv, state),
        Field::Plugins => edit_plugins(siv, state),
        Field::OpenAi => show_provider_menu(siv, state, ProviderSlot::OpenAi),
        Field::Anthropic => show_provider_menu(siv, state, ProviderSlot::Anthropic),
        Field::Ollama => show_provider_menu(siv, state, ProviderSlot::Ollama),
        Field::Thinking => {
            state.lock().unwrap().config.thinking ^= true;
            let idx = FIELDS.iter().position(|f| matches!(f, Field::Thinking)).unwrap();
            show_main_menu(siv, state);
            siv.call_on_name("main_menu", |select: &mut SelectView<Field>| {
                select.set_selection(idx);
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Scalar editors
// ---------------------------------------------------------------------------

/// A Save callback that's shared between the EditView's Enter handler and the
/// Save button on the surrounding Dialog. Cursive callbacks require
/// `Send + Sync + 'static`, so we use an Arc of a trait object.
type SubmitFn = Arc<dyn Fn(&mut Cursive) + Send + Sync + 'static>;

fn edit_usize(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    field: Field,
    current: usize,
    apply: impl Fn(&mut Config, usize) + Send + Sync + 'static,
) {
    let input_name = "scalar_input";
    let state2 = state.clone();
    let apply = Arc::new(apply);

    let on_save: SubmitFn = Arc::new(move |s: &mut Cursive| {
        let value: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        match value.trim().parse::<usize>() {
            Ok(n) => {
                apply(&mut state2.lock().unwrap().config, n);
                show_main_menu(s, state2.clone());
            }
            Err(_) => s.add_layer(Dialog::info("Enter a non-negative integer")),
        }
    });

    let edit = EditView::new()
        .content(current.to_string())
        .on_submit({
            let on_save = on_save.clone();
            move |s, _| on_save(s)
        })
        .with_name(input_name)
        .fixed_width(20);

    let body = LinearLayout::vertical()
        .child(TextView::new(field.description()))
        .child(TextView::new("\nValue:"))
        .child(edit);
    let on_save_button = on_save.clone();
    push_detail(siv, state, field.label(), body, move |s| on_save_button(s));
}

fn edit_optional_f32(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    field: Field,
    current: Option<f32>,
    apply: impl Fn(&mut Config, Option<f32>) + Send + Sync + 'static,
) {
    let input_name = "scalar_input";
    let state2 = state.clone();
    let apply = Arc::new(apply);

    let on_save: SubmitFn = Arc::new(move |s: &mut Cursive| {
        let raw: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        let trimmed = raw.trim();
        let parsed = if trimmed.is_empty() {
            Ok(None)
        } else {
            trimmed.parse::<f32>().map(Some)
        };
        match parsed {
            Ok(v) => {
                apply(&mut state2.lock().unwrap().config, v);
                show_main_menu(s, state2.clone());
            }
            Err(_) => s.add_layer(Dialog::info("Enter a float or leave blank")),
        }
    });

    let edit = EditView::new()
        .content(current.map(|v| v.to_string()).unwrap_or_default())
        .on_submit({
            let on_save = on_save.clone();
            move |s, _| on_save(s)
        })
        .with_name(input_name)
        .fixed_width(20);

    let body = LinearLayout::vertical()
        .child(TextView::new(field.description()))
        .child(TextView::new("\nValue (blank to unset):"))
        .child(edit);
    let on_save_button = on_save.clone();
    push_detail(siv, state, field.label(), body, move |s| on_save_button(s));
}

fn edit_optional_string(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    field: Field,
    current: Option<String>,
    apply: impl Fn(&mut Config, Option<String>) + Send + Sync + 'static,
) {
    let input_name = "scalar_input";
    let state2 = state.clone();
    let apply = Arc::new(apply);

    let on_save: SubmitFn = Arc::new(move |s: &mut Cursive| {
        let raw: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        let value = if raw.trim().is_empty() {
            None
        } else {
            Some(raw)
        };
        apply(&mut state2.lock().unwrap().config, value);
        show_main_menu(s, state2.clone());
    });

    let edit = EditView::new()
        .content(current.unwrap_or_default())
        .on_submit({
            let on_save = on_save.clone();
            move |s, _| on_save(s)
        })
        .with_name(input_name)
        .full_width();

    let body = LinearLayout::vertical()
        .child(TextView::new(field.description()))
        .child(TextView::new("\nValue (blank to unset):"))
        .child(edit);
    let on_save_button = on_save.clone();
    push_detail(siv, state, field.label(), body, move |s| on_save_button(s));
}

fn edit_multiline_string(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    field: Field,
    current: Option<String>,
    apply: impl Fn(&mut Config, Option<String>) + Send + Sync + 'static,
) {
    let input_name = "ml_input";
    let state2 = state.clone();
    let apply = Arc::new(apply);

    let on_save: SubmitFn = Arc::new(move |s: &mut Cursive| {
        let raw: String = s
            .call_on_name(input_name, |v: &mut TextArea| v.get_content().to_string())
            .unwrap_or_default();
        let value = if raw.trim().is_empty() {
            None
        } else {
            Some(raw)
        };
        apply(&mut state2.lock().unwrap().config, value);
        show_main_menu(s, state2.clone());
    });

    let mut textarea = TextArea::new();
    textarea.set_content(current.unwrap_or_default());
    let textarea = textarea.with_name(input_name).min_size((60, 10));

    let body = LinearLayout::vertical()
        .child(TextView::new(field.description()))
        .child(TextView::new("\nValue (blank to unset):"))
        .child(textarea);
    let on_save_button = on_save.clone();
    push_detail(siv, state, field.label(), body, move |s| on_save_button(s));
}


const CONTEXT_FILES_LIST: &str = "context_files_list";

fn edit_context_files(siv: &mut Cursive, state: Arc<Mutex<State>>) {
    let current: Vec<String> = state
        .lock()
        .unwrap()
        .config
        .context_files
        .iter()
        .map(|p| p.display().to_string())
        .collect();

    let mut list: SelectView<String> = SelectView::new();
    for path in &current {
        list.add_item(path.clone(), path.clone());
    }
    let list = vim_keys(list.with_name(CONTEXT_FILES_LIST));

    let state_add = state.clone();
    let state_remove = state.clone();
    let state_save = state.clone();
    let state_cancel = state.clone();

    let body = LinearLayout::vertical()
        .child(TextView::new(
            "File paths injected as high-priority context. \
             Add paths one at a time; select and Remove to delete.",
        ))
        .child(TextView::new(""))
        .child(list.scrollable().min_height(8));

    let dialog = Dialog::around(ResizedView::with_min_width(60, body))
        .title("context_files")
        .button("Add", move |s| {
            show_add_context_file(s, state_add.clone());
        })
        .button("Remove", move |s| {
            let selected: Option<String> = s
                .call_on_name(CONTEXT_FILES_LIST, |v: &mut SelectView<String>| {
                    v.selection().map(|arc| (*arc).clone())
                })
                .flatten();
            if let Some(path) = selected {
                {
                    let mut st = state_remove.lock().unwrap();
                    st.config
                        .context_files
                        .retain(|p| p.display().to_string() != path);
                }
                s.call_on_name(CONTEXT_FILES_LIST, |v: &mut SelectView<String>| {
                    let idx = v.iter().position(|(_, val)| val == &path);
                    if let Some(idx) = idx {
                        v.remove_item(idx);
                    }
                });
            }
        })
        .button("Save", move |s| {
            save_and_notify(s, &state_save);
        })
        .button("Back", move |s| show_main_menu(s, state_cancel.clone()));

    let state_esc = state.clone();
    let wrapped = OnEventView::new(dialog).on_event(Event::Key(Key::Esc), move |s| {
        show_main_menu(s, state_esc.clone())
    });
    siv.pop_layer();
    siv.add_layer(wrapped);
}

fn show_add_context_file(siv: &mut Cursive, state: Arc<Mutex<State>>) {
    let input_name = "new_file_path";
    let state2 = state.clone();

    let on_add: SubmitFn = Arc::new(move |s: &mut Cursive| {
        let raw: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let path = std::path::PathBuf::from(trimmed);
            let path_str = path.display().to_string();
            {
                let mut st = state2.lock().unwrap();
                if !st.config.context_files.contains(&path) {
                    st.config.context_files.push(path);
                }
            }
            s.call_on_name(CONTEXT_FILES_LIST, |v: &mut SelectView<String>| {
                v.add_item(path_str.clone(), path_str);
            });
        }
        s.pop_layer();
    });

    let edit = EditView::new()
        .on_submit({
            let on_add = on_add.clone();
            move |s, _| on_add(s)
        })
        .with_name(input_name)
        .full_width();

    let body = LinearLayout::vertical()
        .child(TextView::new("Enter the absolute path to the file:"))
        .child(TextView::new(""))
        .child(edit);

    let on_add_button = on_add.clone();
    let dialog = Dialog::around(ResizedView::with_min_width(60, body))
        .title("Add context file")
        .button("Add", move |s| on_add_button(s))
        .button("Cancel", |s| {
            s.pop_layer();
        });

    siv.add_layer(dialog);
}

const PLUGINS_LIST: &str = "plugins_list";

fn edit_plugins(siv: &mut Cursive, state: Arc<Mutex<State>>) {
    let current: Vec<String> = state.lock().unwrap().config.plugins.clone();

    let mut list: SelectView<String> = SelectView::new();
    for cmd in &current {
        list.add_item(cmd.clone(), cmd.clone());
    }
    let list = vim_keys(list.with_name(PLUGINS_LIST));

    let state_add = state.clone();
    let state_remove = state.clone();
    let state_save = state.clone();
    let state_cancel = state.clone();

    let body = LinearLayout::vertical()
        .child(TextView::new(
            "Shell commands run before each LLM request. \
             Their stdout is injected into the prompt as additional context.",
        ))
        .child(TextView::new(""))
        .child(list.scrollable().min_height(8));

    let dialog = Dialog::around(ResizedView::with_min_width(60, body))
        .title("plugins")
        .button("Add", move |s| {
            show_add_plugin(s, state_add.clone());
        })
        .button("Remove", move |s| {
            let selected: Option<String> = s
                .call_on_name(PLUGINS_LIST, |v: &mut SelectView<String>| {
                    v.selection().map(|arc| (*arc).clone())
                })
                .flatten();
            if let Some(cmd) = selected {
                {
                    let mut st = state_remove.lock().unwrap();
                    st.config.plugins.retain(|c| c != &cmd);
                }
                s.call_on_name(PLUGINS_LIST, |v: &mut SelectView<String>| {
                    let idx = v.iter().position(|(_, val)| val == &cmd);
                    if let Some(idx) = idx {
                        v.remove_item(idx);
                    }
                });
            }
        })
        .button("Save", move |s| {
            save_and_notify(s, &state_save);
        })
        .button("Back", move |s| show_main_menu(s, state_cancel.clone()));

    let state_esc = state.clone();
    let wrapped = OnEventView::new(dialog).on_event(Event::Key(Key::Esc), move |s| {
        show_main_menu(s, state_esc.clone())
    });
    siv.pop_layer();
    siv.add_layer(wrapped);
}

fn show_add_plugin(siv: &mut Cursive, state: Arc<Mutex<State>>) {
    let input_name = "new_plugin_cmd";
    let state2 = state.clone();

    let on_add: SubmitFn = Arc::new(move |s: &mut Cursive| {
        let raw: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        let trimmed = raw.trim().to_string();
        if !trimmed.is_empty() {
            {
                let mut st = state2.lock().unwrap();
                if !st.config.plugins.contains(&trimmed) {
                    st.config.plugins.push(trimmed.clone());
                }
            }
            s.call_on_name(PLUGINS_LIST, |v: &mut SelectView<String>| {
                v.add_item(trimmed.clone(), trimmed);
            });
        }
        s.pop_layer();
    });

    let edit = EditView::new()
        .on_submit({
            let on_add = on_add.clone();
            move |s, _| on_add(s)
        })
        .with_name(input_name)
        .full_width();

    let body = LinearLayout::vertical()
        .child(TextView::new("Enter the shell command to run as a plugin:"))
        .child(TextView::new(""))
        .child(edit);

    let on_add_button = on_add.clone();
    let dialog = Dialog::around(ResizedView::with_min_width(60, body))
        .title("Add plugin")
        .button("Add", move |s| on_add_button(s))
        .button("Cancel", |s| {
            s.pop_layer();
        });

    siv.add_layer(dialog);
}

fn push_detail<V: cursive::view::View>(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    title: &str,
    body: V,
    on_save: impl Fn(&mut Cursive) + Send + Sync + 'static,
) {
    let dialog = Dialog::around(ResizedView::with_min_width(50, body))
        .title(title)
        .button("Save", on_save)
        .button("Cancel", {
            let state = state.clone();
            move |s| show_main_menu(s, state.clone())
        });
    let wrapped = OnEventView::new(dialog).on_event(Event::Key(Key::Esc), {
        let state = state.clone();
        move |s| show_main_menu(s, state.clone())
    });
    siv.pop_layer();
    siv.add_layer(wrapped);
}

// ---------------------------------------------------------------------------
// Provider sub-menu (api_key / base_url)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
enum ProviderSlot {
    OpenAi,
    Anthropic,
    Ollama,
}
impl ProviderSlot {
    fn name(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Ollama => "ollama",
        }
    }
    fn get<'a>(&self, cfg: &'a Config) -> Option<&'a ProviderEnv> {
        match self {
            Self::OpenAi => cfg.providers.openai.as_ref(),
            Self::Anthropic => cfg.providers.anthropic.as_ref(),
            Self::Ollama => cfg.providers.ollama.as_ref(),
        }
    }
    fn set(&self, cfg: &mut Config, env: ProviderEnv) {
        let slot = match self {
            Self::OpenAi => &mut cfg.providers.openai,
            Self::Anthropic => &mut cfg.providers.anthropic,
            Self::Ollama => &mut cfg.providers.ollama,
        };
        *slot = Some(env);
    }
}

fn show_provider_menu(siv: &mut Cursive, state: Arc<Mutex<State>>, slot: ProviderSlot) {
    let mut select: SelectView<&'static str> = SelectView::new();
    select.add_item("api_key", "api_key");
    select.add_item("base_url", "base_url");
    let state_for_submit = state.clone();
    select.set_on_submit(move |s, which: &&'static str| {
        let key = *which;
        edit_provider_field(s, state_for_submit.clone(), slot, key);
    });
    let select = vim_keys(select);

    let title = format!("providers.{}", slot.name());
    let dialog = Dialog::around(select.scrollable().full_width().min_height(6))
        .title(title)
        .button("Back", {
            let state = state.clone();
            move |s| show_main_menu(s, state.clone())
        });
    let wrapped = OnEventView::new(dialog).on_event(Event::Key(Key::Esc), {
        let state = state.clone();
        move |s| show_main_menu(s, state.clone())
    });
    siv.pop_layer();
    siv.add_layer(wrapped);
}

fn edit_provider_field(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    slot: ProviderSlot,
    which: &'static str,
) {
    let current = slot
        .get(&state.lock().unwrap().config)
        .and_then(|env| match which {
            "api_key" => env.api_key.clone(),
            "base_url" => env.base_url.clone(),
            _ => None,
        })
        .unwrap_or_default();

    let input_name = "scalar_input";
    let state2 = state.clone();

    let on_save: SubmitFn = Arc::new(move |s: &mut Cursive| {
        let raw: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        let value = if raw.trim().is_empty() {
            None
        } else {
            Some(raw)
        };
        {
            let mut st = state2.lock().unwrap();
            let mut env = slot.get(&st.config).cloned().unwrap_or_default();
            match which {
                "api_key" => env.api_key = value,
                "base_url" => env.base_url = value,
                _ => {}
            }
            slot.set(&mut st.config, env);
        }
        show_provider_menu(s, state2.clone(), slot);
    });

    let edit = EditView::new()
        .content(current)
        .on_submit({
            let on_save = on_save.clone();
            move |s, _| on_save(s)
        })
        .with_name(input_name)
        .full_width();

    let title = format!("providers.{}.{which}", slot.name());
    let body = LinearLayout::vertical()
        .child(TextView::new(format!(
            "Edit `{which}` for `{}` (blank to unset).",
            slot.name()
        )))
        .child(TextView::new("\nValue:"))
        .child(edit);
    let on_save_button = on_save.clone();
    let dialog = Dialog::around(ResizedView::with_min_width(50, body))
        .title(title)
        .button("Save", move |s| on_save_button(s))
        .button("Cancel", {
            let state = state.clone();
            move |s| show_provider_menu(s, state.clone(), slot)
        });
    let wrapped = OnEventView::new(dialog).on_event(Event::Key(Key::Esc), {
        let state = state.clone();
        move |s| show_provider_menu(s, state.clone(), slot)
    });
    siv.pop_layer();
    siv.add_layer(wrapped);
}

// ---------------------------------------------------------------------------
// Model picker (`--model-list` output)
// ---------------------------------------------------------------------------

fn show_model_picker(siv: &mut Cursive, state: Arc<Mutex<State>>) {
    // Fetch synchronously; the TUI briefly freezes while the catalogue loads.
    let result = model_list::collect_rows();
    let rows = match result {
        Ok(r) => r,
        Err(e) => {
            let state = state.clone();
            siv.add_layer(
                Dialog::text(format!("Failed to load model list:\n{e:#}"))
                    .button("Back", move |s| show_main_menu(s, state.clone())),
            );
            return;
        }
    };
    let rows = Arc::new(rows);

    // Load the current value from the config
    let current = state.lock().unwrap().config.model.clone();

    let mut select: SelectView<String> = SelectView::new();
    populate_models(&mut select, &rows, "");
    let state_for_submit = state.clone();
    select.set_on_submit(move |s, model: &String| {
        state_for_submit.lock().unwrap().config.model = model.clone();
        show_main_menu(s, state_for_submit.clone());
    });
    let select = select.with_name(MODEL_LIST_NAME);

    let filter_edit = EditView::new()
        .content(current)
        .on_edit({
            let rows = rows.clone();
            move |s, content, _cursor| {
                s.call_on_name(MODEL_LIST_NAME, |v: &mut SelectView<String>| {
                    populate_models(v, &rows, content);
                });
            }
        })
        .on_submit({
            // Enter from the filter: focus the list so the user can pick.
            |s, _| {
                let _ = s.focus_name(MODEL_LIST_NAME);
            }
        })
        .with_name(MODEL_FILTER_NAME)
        .full_width();

    let body = LinearLayout::vertical()
        .child(TextView::new(
            "Type to filter, enter to confirm filter, then ↑/↓ or j/k to pick. \
             Esc cancels.",
        ))
        .child(Panel::new(filter_edit).title("filter"))
        .child(
            vim_keys_named::<SelectView<String>>(select)
                .scrollable()
                .min_height(15),
        );

    let state_back = state.clone();
    let dialog = Dialog::around(ResizedView::with_min_width(60, body))
        .title("Pick a model")
        .button("Back", move |s| show_main_menu(s, state_back.clone()));

    let state_esc = state.clone();
    // `/` focuses the filter edit; Esc returns to the main menu.
    let wrapped = OnEventView::new(dialog)
        .on_event('/', |s| {
            let _ = s.focus_name(MODEL_FILTER_NAME);
        })
        .on_event(Event::Key(Key::Esc), move |s| {
            show_main_menu(s, state_esc.clone())
        });
    siv.pop_layer();
    siv.add_layer(wrapped);
}

fn populate_models(select: &mut SelectView<String>, rows: &[Row], filter: &str) {
    select.clear();
    let needle = filter.to_lowercase();
    let label_w = rows.iter().map(|r| r.qualified.len()).max().unwrap_or(0);
    for row in rows {
        if !needle.is_empty() && !row.qualified.to_lowercase().contains(&needle) {
            continue;
        }
        let label = format!("{:<w$}  {}", row.qualified, row.price_label(), w = label_w);
        select.add_item(label, row.qualified.clone());
    }
}

// ---------------------------------------------------------------------------
// Vim navigation helpers
// ---------------------------------------------------------------------------

fn vim_keys<V: cursive::view::View>(v: V) -> OnEventView<V> {
    OnEventView::new(v)
        .on_pre_event(Event::Char('j'), |s| {
            s.on_event(Event::Key(Key::Down));
        })
        .on_pre_event(Event::Char('k'), |s| {
            s.on_event(Event::Key(Key::Up));
        })
        .on_pre_event(Event::Char('l'), |s| {
            s.on_event(Event::Key(Key::Enter));
        })
        .on_pre_event(Event::Char('h'), |s| {
            s.on_event(Event::Key(Key::Esc));
        })
}

fn vim_keys_named<V: cursive::view::View + 'static>(
    v: cursive::views::NamedView<V>,
) -> OnEventView<cursive::views::NamedView<V>> {
    OnEventView::new(v)
        .on_pre_event(Event::Char('j'), |s| {
            s.on_event(Event::Key(Key::Down));
        })
        .on_pre_event(Event::Char('k'), |s| {
            s.on_event(Event::Key(Key::Up));
        })
}

// ---------------------------------------------------------------------------
// Save
// ---------------------------------------------------------------------------

fn save_and_notify(siv: &mut Cursive, state: &Arc<Mutex<State>>) {
    let st = state.lock().unwrap();
    match st.config.save(&st.path) {
        Ok(()) => {
            let path = st.path.display().to_string();
            siv.add_layer(Dialog::text(format!("Saved to {path}")).button("OK", |s| {
                s.pop_layer();
                s.quit()
            }));
        }
        Err(e) => {
            siv.add_layer(Dialog::info(format!("Save failed: {e:#}")));
        }
    }
}
