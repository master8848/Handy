use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

fn parse_var(s: &str) -> Result<(String, String), String> {
    let mut parts = s.splitn(2, '=');
    let k = parts.next().ok_or("missing key")?;
    let v = parts.next().ok_or("missing value, expected k=v")?;
    if k.is_empty() {
        return Err("empty variable name".to_string());
    }
    Ok((k.to_string(), v.to_string()))
}

#[derive(ValueEnum, Debug, Clone)]
pub enum PromptSortArg {
    UpdatedDesc,
    CreatedDesc,
    UsageDesc,
    TitleAsc,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PromptCmd {
    /// Search prompts via FTS5 (bm25 rank, pinned first). Empty query falls back to list.
    Search {
        /// Query string (FTS5 MATCH); supports quoted phrases
        query: String,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        folder: Option<i64>,
        #[arg(long, default_value = "20")]
        limit: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        copy: bool,
        #[arg(long)]
        raw: bool,
    },
    /// List prompts with filters
    List {
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        folder: Option<i64>,
        #[arg(long)]
        pinned: bool,
        #[arg(long, value_enum)]
        sort: Option<PromptSortArg>,
        #[arg(long, default_value = "20")]
        limit: usize,
        #[arg(long)]
        offset: Option<i64>,
        #[arg(long)]
        json: bool,
    },
    /// Get a single prompt by id
    Get {
        id: i64,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        raw: bool,
        #[arg(long, value_parser=parse_var)]
        var: Vec<(String, String)>,
        #[arg(long)]
        stdout: bool,
    },
    /// Resolve by id or fuzzy title search and output (or copy) the rendered prompt
    Use {
        /// Prompt id or search query
        query: String,
        #[arg(long)]
        id: Option<i64>,
        #[arg(long, value_parser=parse_var)]
        var: Vec<(String, String)>,
        #[arg(long)]
        stdout: bool,
        #[arg(long)]
        copy: bool,
        #[arg(long)]
        json: bool,
    },
    /// Create a prompt (content from --content or stdin)
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        folder: Option<i64>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum SkillCmd {
    /// Export a prompt as an Agent Skill (SKILL.md)
    Export {
        id: i64,
        #[arg(long, default_value = "skill")]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        stdout: bool,
    },
    /// Import a SKILL.md / prompt file into the library
    Add {
        source: String,
        #[arg(long)]
        folder: Option<i64>,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Local search (alias for prompt search)
    Find {
        query: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value = "20")]
        limit: usize,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Prompt library commands (search, list, get, use, create)
    Prompt {
        #[command(subcommand)]
        cmd: PromptCmd,
    },
    /// Skill interop (export/add/find) — Agent Skills SKILL.md ↔ Handy library
    Skill {
        #[command(subcommand)]
        cmd: SkillCmd,
    },
}

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "handy", about = "Handy - Speech to Text")]
pub struct CliArgs {
    /// Start with the main window hidden
    #[arg(long)]
    pub start_hidden: bool,

    /// Disable the system tray icon
    #[arg(long)]
    pub no_tray: bool,

    /// Toggle transcription on/off (sent to running instance)
    #[arg(long)]
    pub toggle_transcription: bool,

    /// Toggle transcription with post-processing on/off (sent to running instance)
    #[arg(long)]
    pub toggle_post_process: bool,

    /// Cancel the current operation (sent to running instance)
    #[arg(long)]
    pub cancel: bool,

    /// Enable debug mode with verbose logging
    #[arg(long)]
    pub debug: bool,

    /// Transcribe this WAV (16 kHz mono) headlessly and exit. Runs the same
    /// batch transcription path as the app — no mic, no VAD, no download
    /// (the model must already be installed).
    #[arg(short = 'f', long, value_name = "WAV")]
    pub transcribe_file: Option<PathBuf>,

    /// Model id to load for --transcribe-file (default: the selected model).
    #[arg(long)]
    pub model: Option<String>,

    /// Hard-select the compute device for --transcribe-file by its registry
    /// index (see --list-devices). Omit to use the persisted accelerator
    /// setting. transcribe-cpp (whisper-family) models only.
    #[arg(long, value_name = "N")]
    pub device_index: Option<usize>,

    /// List the transcribe-cpp compute devices (with indices) and exit.
    #[arg(long)]
    pub list_devices: bool,

    /// List the available models (with ids) and exit. Pass an id to --model.
    /// Honors --json for machine-readable output.
    #[arg(long)]
    pub list_models: bool,

    /// Repeat the transcription N times (best_ms reports the fastest run).
    #[arg(long, value_name = "N")]
    pub repeat: Option<usize>,

    /// Emit --transcribe-file results as JSON.
    #[arg(long)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}
