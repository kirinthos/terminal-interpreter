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
use cursive::event::{Event, Key};
use cursive::traits::*;
use cursive::view::Nameable;
use cursive::views::{
    Dialog, EditView, LinearLayout, OnEventView, Panel, ResizedView, SelectView, TextView,
};
use cursive::Cursive;

use crate::config::{Config, ProviderEnv};
use crate::model_list::{self, Row};

const MODEL_LIST_NAME: &str = "model_list";
const MODEL_FILTER_NAME: &str = "model_filter";

pub fn run(path: PathBuf, config: Config) -> Result<()> {
    let state = Arc::new(Mutex::new(State { config, path }));
    let mut siv = cursive::default();
    show_main_menu(&mut siv, state);
    siv.run();
    Ok(())
}

struct State {
    config: Config,
    path: PathBuf,
}

// ---------------------------------------------------------------------------
// Field schema — single source of truth for menu entries and descriptions.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
enum Field {
    Model,
    HistoryLimit,
    HistoryReadLimit,
    Temperature,
    SystemPrompt,
    OpenAi,
    Anthropic,
    Ollama,
}

impl Field {
    fn label(&self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::HistoryLimit => "history_limit",
            Self::HistoryReadLimit => "history_read_limit",
            Self::Temperature => "temperature",
            Self::SystemPrompt => "system_prompt",
            Self::OpenAi => "providers.openai",
            Self::Anthropic => "providers.anthropic",
            Self::Ollama => "providers.ollama",
        }
    }
    fn description(&self) -> &'static str {
        match self {
            Self::Model =>
                "Default model in `provider/model-name` form. Pick from a live catalogue.",
            Self::HistoryLimit =>
                "How many recent shell-history lines are sent to the LLM as context.",
            Self::HistoryReadLimit =>
                "How many lines to read off the tail of the shell history file before trimming.",
            Self::Temperature =>
                "Sampling temperature (0.0–2.0). Leave blank to use the provider default.",
            Self::SystemPrompt =>
                "Override the system prompt sent to the model. Leave blank for the default.",
            Self::OpenAi => "OpenAI provider settings: api_key, base_url.",
            Self::Anthropic => "Anthropic provider settings: api_key, base_url.",
            Self::Ollama => "Ollama provider settings: api_key (usually unset), base_url.",
        }
    }
}

const FIELDS: &[Field] = &[
    Field::Model,
    Field::HistoryLimit,
    Field::HistoryReadLimit,
    Field::Temperature,
    Field::SystemPrompt,
    Field::OpenAi,
    Field::Anthropic,
    Field::Ollama,
];

// ---------------------------------------------------------------------------
// Main menu
// ---------------------------------------------------------------------------

fn show_main_menu(siv: &mut Cursive, state: Arc<Mutex<State>>) {
    let mut select: SelectView<Field> = SelectView::new();
    let label_w = FIELDS.iter().map(|f| f.label().len()).max().unwrap_or(0);
    for f in FIELDS {
        let label = format!("{:<w$}  {}", f.label(), f.description(), w = label_w);
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
        Field::HistoryReadLimit => edit_usize(
            siv,
            state.clone(),
            field,
            state.lock().unwrap().config.history_read_limit,
            |c, v| c.history_read_limit = v,
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
        Field::OpenAi => show_provider_menu(siv, state, ProviderSlot::OpenAi),
        Field::Anthropic => show_provider_menu(siv, state, ProviderSlot::Anthropic),
        Field::Ollama => show_provider_menu(siv, state, ProviderSlot::Ollama),
    }
}

// ---------------------------------------------------------------------------
// Scalar editors
// ---------------------------------------------------------------------------

fn edit_usize(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    field: Field,
    current: usize,
    apply: impl Fn(&mut Config, usize) + Send + Sync + 'static,
) {
    let input_name = "scalar_input";
    let edit = EditView::new()
        .content(current.to_string())
        .with_name(input_name)
        .fixed_width(20);

    let apply = Arc::new(apply);
    let state2 = state.clone();
    let apply2 = apply.clone();
    let on_save = move |s: &mut Cursive| {
        let value: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        match value.trim().parse::<usize>() {
            Ok(n) => {
                apply2(&mut state2.lock().unwrap().config, n);
                show_main_menu(s, state2.clone());
            }
            Err(_) => s.add_layer(Dialog::info("Enter a non-negative integer")),
        }
    };

    let body = LinearLayout::vertical()
        .child(TextView::new(field.description()))
        .child(TextView::new("\nValue:"))
        .child(edit);
    push_detail(siv, state, field.label(), body, on_save);
}

fn edit_optional_f32(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    field: Field,
    current: Option<f32>,
    apply: impl Fn(&mut Config, Option<f32>) + Send + Sync + 'static,
) {
    let input_name = "scalar_input";
    let edit = EditView::new()
        .content(current.map(|v| v.to_string()).unwrap_or_default())
        .with_name(input_name)
        .fixed_width(20);

    let apply = Arc::new(apply);
    let state2 = state.clone();
    let apply2 = apply.clone();
    let on_save = move |s: &mut Cursive| {
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
                apply2(&mut state2.lock().unwrap().config, v);
                show_main_menu(s, state2.clone());
            }
            Err(_) => s.add_layer(Dialog::info("Enter a float or leave blank")),
        }
    };

    let body = LinearLayout::vertical()
        .child(TextView::new(field.description()))
        .child(TextView::new("\nValue (blank to unset):"))
        .child(edit);
    push_detail(siv, state, field.label(), body, on_save);
}

fn edit_optional_string(
    siv: &mut Cursive,
    state: Arc<Mutex<State>>,
    field: Field,
    current: Option<String>,
    apply: impl Fn(&mut Config, Option<String>) + Send + Sync + 'static,
) {
    let input_name = "scalar_input";
    let edit = EditView::new()
        .content(current.unwrap_or_default())
        .with_name(input_name)
        .full_width();

    let apply = Arc::new(apply);
    let state2 = state.clone();
    let apply2 = apply.clone();
    let on_save = move |s: &mut Cursive| {
        let raw: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        let value = if raw.trim().is_empty() {
            None
        } else {
            Some(raw)
        };
        apply2(&mut state2.lock().unwrap().config, value);
        show_main_menu(s, state2.clone());
    };

    let body = LinearLayout::vertical()
        .child(TextView::new(field.description()))
        .child(TextView::new("\nValue (blank to unset):"))
        .child(edit);
    push_detail(siv, state, field.label(), body, on_save);
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
    let edit = EditView::new()
        .content(current)
        .with_name(input_name)
        .full_width();

    let state2 = state.clone();
    let on_save = move |s: &mut Cursive| {
        let raw: String = s
            .call_on_name(input_name, |v: &mut EditView| v.get_content().to_string())
            .unwrap_or_default();
        let value = if raw.trim().is_empty() { None } else { Some(raw) };
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
    };

    let title = format!("providers.{}.{which}", slot.name());
    let body = LinearLayout::vertical()
        .child(TextView::new(format!(
            "Edit `{which}` for `{}` (blank to unset).",
            slot.name()
        )))
        .child(TextView::new("\nValue:"))
        .child(edit);
    let dialog = Dialog::around(ResizedView::with_min_width(50, body))
        .title(title)
        .button("Save", on_save)
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
// Model picker (`--model-list` output, `/`-to-filter)
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

    let mut select: SelectView<String> = SelectView::new();
    populate_models(&mut select, &rows, "");
    let state_for_submit = state.clone();
    select.set_on_submit(move |s, model: &String| {
        state_for_submit.lock().unwrap().config.model = model.clone();
        show_main_menu(s, state_for_submit.clone());
    });
    let select = select.with_name(MODEL_LIST_NAME);

    let filter_edit = EditView::new()
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
            "Press `/` to search, Enter to confirm filter, then ↑/↓ or j/k to pick. \
             Esc cancels.",
        ))
        .child(Panel::new(filter_edit).title("filter"))
        .child(vim_keys_named::<SelectView<String>>(select).scrollable().min_height(15));

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
        let label = format!(
            "{:<w$}  {}",
            row.qualified,
            row.price_label(),
            w = label_w
        );
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
            siv.add_layer(
                Dialog::text(format!("Saved to {path}")).button("OK", |s| {
                    s.pop_layer();
                }),
            );
        }
        Err(e) => {
            siv.add_layer(Dialog::info(format!("Save failed: {e:#}")));
        }
    }
}
