use rand::Rng;
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ENV_LOCAL_FILE: &str = ".env.local";
const LEGACY_ENV_FILE: &str = ".env";
const APPDATA_SUBDIR: &str = "Brido";

const ENV_TEMPLATE: &str = "# Brido AI provider configuration\n\
# This file is created automatically on first run.\n\
# Keep this file private. Do not commit it to git.\n\
\n\
OPENAI_API_KEY=\n\
OPENAI_BASE_URL=https://api.openai.com/v1\n\
OPENAI_MODEL=gpt-4.1-mini\n\
\n\
ANTHROPIC_API_KEY=\n\
ANTHROPIC_BASE_URL=https://api.anthropic.com/v1\n\
ANTHROPIC_MODEL=claude-3-5-sonnet-latest\n\
\n\
GEMINI_API_KEY=\n\
GEMINI_BASE_URL=https://generativelanguage.googleapis.com/v1beta\n\
GEMINI_MODEL=gemini-2.0-flash\n\
\n\
OPENROUTER_API_KEY=\n\
OPENROUTER_BASE_URL=https://openrouter.ai/api/v1\n\
OPENROUTER_MODEL=google/gemini-2.5-flash\n";

const MANAGED_ENV_KEYS: [&str; 12] = [
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "OPENAI_MODEL",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_MODEL",
    "GEMINI_API_KEY",
    "GEMINI_BASE_URL",
    "GEMINI_MODEL",
    "OPENROUTER_API_KEY",
    "OPENROUTER_BASE_URL",
    "OPENROUTER_MODEL",
];

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub pin: String,
    pub capture_fps: u32,
    pub capture_quality: u8,
    pub target_width: u32,
    pub target_height: u32,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub anthropic_api_key: String,
    pub anthropic_base_url: String,
    pub anthropic_model: String,
    pub gemini_api_key: String,
    pub gemini_base_url: String,
    pub gemini_model: String,
    pub openrouter_api_key: String,
    pub openrouter_base_url: String,
    pub openrouter_model: String,
}

impl Config {
    pub fn has_any_provider_key(&self) -> bool {
        !self.openai_api_key.trim().is_empty()
            || !self.anthropic_api_key.trim().is_empty()
            || !self.gemini_api_key.trim().is_empty()
            || !self.openrouter_api_key.trim().is_empty()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            pin: generate_pin(),
            capture_fps: 15,
            capture_quality: 65,
            target_width: 1280,
            target_height: 720,
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_default(),
            openai_base_url: env_or_default("OPENAI_BASE_URL", "https://api.openai.com/v1"),
            openai_model: env_or_default("OPENAI_MODEL", "gpt-4.1-mini"),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            anthropic_base_url: env_or_default("ANTHROPIC_BASE_URL", "https://api.anthropic.com/v1"),
            anthropic_model: env_or_default("ANTHROPIC_MODEL", "claude-3-5-sonnet-latest"),
            gemini_api_key: env::var("GEMINI_API_KEY").unwrap_or_default(),
            gemini_base_url: env_or_default("GEMINI_BASE_URL", "https://generativelanguage.googleapis.com/v1beta"),
            gemini_model: env_or_default("GEMINI_MODEL", "gemini-2.0-flash"),
            openrouter_api_key: env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            openrouter_base_url: env_or_default("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"),
            openrouter_model: env_or_default("OPENROUTER_MODEL", "google/gemini-2.5-flash"),
        }
    }
}

fn env_or_default(key: &str, fallback: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Gemini,
    OpenRouter,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 4] = [
        ProviderKind::OpenAI,
        ProviderKind::Anthropic,
        ProviderKind::Gemini,
        ProviderKind::OpenRouter,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "OpenAI",
            ProviderKind::Anthropic => "Anthropic",
            ProviderKind::Gemini => "Gemini",
            ProviderKind::OpenRouter => "OpenRouter",
        }
    }

    pub fn api_key_var(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "OPENAI_API_KEY",
            ProviderKind::Anthropic => "ANTHROPIC_API_KEY",
            ProviderKind::Gemini => "GEMINI_API_KEY",
            ProviderKind::OpenRouter => "OPENROUTER_API_KEY",
        }
    }

    pub fn base_url_var(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "OPENAI_BASE_URL",
            ProviderKind::Anthropic => "ANTHROPIC_BASE_URL",
            ProviderKind::Gemini => "GEMINI_BASE_URL",
            ProviderKind::OpenRouter => "OPENROUTER_BASE_URL",
        }
    }

    pub fn model_var(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "OPENAI_MODEL",
            ProviderKind::Anthropic => "ANTHROPIC_MODEL",
            ProviderKind::Gemini => "GEMINI_MODEL",
            ProviderKind::OpenRouter => "OPENROUTER_MODEL",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "https://api.openai.com/v1",
            ProviderKind::Anthropic => "https://api.anthropic.com/v1",
            ProviderKind::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            ProviderKind::OpenRouter => "https://openrouter.ai/api/v1",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "gpt-4.1-mini",
            ProviderKind::Anthropic => "claude-3-5-sonnet-latest",
            ProviderKind::Gemini => "gemini-2.0-flash",
            ProviderKind::OpenRouter => "google/gemini-2.5-flash",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeEnvPaths {
    pub active_env_path: PathBuf,
    pub primary_env_path: PathBuf,
    pub fallback_env_path: PathBuf,
    pub legacy_env_path: PathBuf,
    pub is_using_fallback: bool,
    pub migrated_legacy_env: bool,
}

#[derive(Debug)]
pub enum EnvConfigError {
    Validation(String),
    PermissionDenied { path: PathBuf, source: io::Error },
    Io { path: PathBuf, source: io::Error },
    Parse(String),
}

impl fmt::Display for EnvConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvConfigError::Validation(msg) => write!(f, "{msg}"),
            EnvConfigError::PermissionDenied { path, source } => {
                write!(f, "Permission denied for {}: {}", path.display(), source)
            }
            EnvConfigError::Io { path, source } => {
                write!(f, "I/O error for {}: {}", path.display(), source)
            }
            EnvConfigError::Parse(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EnvConfigError {}

pub fn bootstrap_runtime_env() -> Result<RuntimeEnvPaths, EnvConfigError> {
    let exe_dir = executable_dir();
    let primary_env_path = exe_dir.join(ENV_LOCAL_FILE);
    let legacy_env_path = exe_dir.join(LEGACY_ENV_FILE);
    let fallback_env_path = fallback_env_path(&exe_dir);

    let mut is_using_fallback = false;
    let mut active_env_path = primary_env_path.clone();
    let primary_exists = primary_env_path.exists();
    let fallback_exists = fallback_env_path.exists();

    let mut active_existed_before = false;

    if primary_exists {
        if can_write_env_file(&primary_env_path) {
            active_env_path = primary_env_path.clone();
            active_existed_before = true;
        } else {
            tracing::warn!(
                "Primary env file is not writable ({}). Using fallback path {}.",
                primary_env_path.display(),
                fallback_env_path.display()
            );
            is_using_fallback = true;
            active_env_path = fallback_env_path.clone();
            active_existed_before = fallback_exists;
        }
    } else if fallback_exists {
        is_using_fallback = true;
        active_env_path = fallback_env_path.clone();
        active_existed_before = true;
    }

    if !active_existed_before {
        match ensure_env_file_exists(&active_env_path) {
            Ok(_) => {
                // Active path successfully created.
            }
            Err(err) => {
                if !is_using_fallback {
                    tracing::warn!(
                        "Primary env path unavailable ({}). Falling back to {}: {}",
                        primary_env_path.display(),
                        fallback_env_path.display(),
                        err
                    );
                    ensure_env_file_exists(&fallback_env_path)?;
                    active_env_path = fallback_env_path.clone();
                    is_using_fallback = true;
                } else {
                    return Err(err);
                }
            }
        }
    } else {
        ensure_env_file_exists(&active_env_path)?;
    }

    let mut migrated_legacy_env = false;
    if !active_existed_before && legacy_env_path.exists() {
        migrated_legacy_env = migrate_legacy_env(&legacy_env_path, &active_env_path)?;
    }

    if legacy_env_path.exists() && active_env_path.exists() && !migrated_legacy_env {
        tracing::warn!(
            "Both .env and .env.local exist. Using {} with .env.local precedence.",
            active_env_path.display()
        );
    }

    let runtime = RuntimeEnvPaths {
        active_env_path,
        primary_env_path,
        fallback_env_path,
        legacy_env_path,
        is_using_fallback,
        migrated_legacy_env,
    };

    load_runtime_env(&runtime)?;

    if debug_enabled() {
        tracing::info!(
            "Runtime env active path: {} (fallback: {}, migrated: {})",
            runtime.active_env_path.display(),
            runtime.is_using_fallback,
            runtime.migrated_legacy_env
        );
    }

    Ok(runtime)
}

pub fn load_runtime_env(runtime: &RuntimeEnvPaths) -> Result<(), EnvConfigError> {
    clear_managed_env_vars();

    if runtime.legacy_env_path.exists() {
        dotenvy::from_path_override(&runtime.legacy_env_path).map_err(|err| {
            EnvConfigError::Parse(format!(
                "Failed to parse {}: {}",
                runtime.legacy_env_path.display(),
                err
            ))
        })?;
    }

    ensure_env_file_exists(&runtime.active_env_path)?;
    dotenvy::from_path_override(&runtime.active_env_path).map_err(|err| {
        EnvConfigError::Parse(format!(
            "Failed to parse {}: {}",
            runtime.active_env_path.display(),
            err
        ))
    })
}

pub fn save_provider_api_key(
    runtime: &RuntimeEnvPaths,
    provider: ProviderKind,
    api_key: &str,
) -> Result<(), EnvConfigError> {
    ensure_env_file_exists(&runtime.active_env_path)?;

    let sanitized_key = sanitize_api_key(api_key)?;
    validate_provider_key(provider, &sanitized_key)?;

    let existing = read_env_file(&runtime.active_env_path)?;
    let line_ending = detect_line_ending(&existing);
    let mut lines = split_lines(&existing);

    for kind in ProviderKind::ALL {
        let value = if kind == provider {
            sanitized_key.as_str()
        } else {
            ""
        };
        upsert_env_line(&mut lines, kind.api_key_var(), value);
    }

    upsert_env_if_missing(&mut lines, provider.base_url_var(), provider.default_base_url());
    upsert_env_if_missing(&mut lines, provider.model_var(), provider.default_model());

    let updated = join_lines(&lines, line_ending);
    write_env_file(&runtime.active_env_path, &updated)?;
    load_runtime_env(runtime)?;

    Ok(())
}

fn generate_pin() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000u32))
}

fn executable_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn fallback_env_path(exe_dir: &Path) -> PathBuf {
    if let Some(appdata) = env::var_os("APPDATA") {
        PathBuf::from(appdata)
            .join(APPDATA_SUBDIR)
            .join(ENV_LOCAL_FILE)
    } else {
        exe_dir.join(APPDATA_SUBDIR).join(ENV_LOCAL_FILE)
    }
}

fn can_write_env_file(path: &Path) -> bool {
    if path.exists() {
        fs::OpenOptions::new().append(true).open(path).is_ok()
    } else {
        path.parent().map(can_write_directory).unwrap_or(false)
    }
}

fn can_write_directory(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    let probe_path = path.join(format!(".write_probe_{}", std::process::id()));
    match fs::write(&probe_path, b"ok") {
        Ok(_) => {
            let _ = fs::remove_file(&probe_path);
            true
        }
        Err(_) => false,
    }
}

fn ensure_env_file_exists(path: &Path) -> Result<bool, EnvConfigError> {
    if path.exists() {
        return Ok(false);
    }

    let parent = path.parent().ok_or_else(|| EnvConfigError::Io {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::NotFound, "Missing parent directory"),
    })?;

    fs::create_dir_all(parent).map_err(|err| map_io_error(parent, err))?;
    write_env_file(path, ENV_TEMPLATE)?;
    Ok(true)
}

fn migrate_legacy_env(legacy_path: &Path, target_path: &Path) -> Result<bool, EnvConfigError> {
    let legacy = read_env_file(legacy_path)?;
    let mut target = read_env_file(target_path)?;

    let target_line_ending = detect_line_ending(&target);
    let mut lines = split_lines(&target);
    let entries = parse_env_entries(&legacy);

    let mut migrated = false;
    for (key, value) in entries {
        if MANAGED_ENV_KEYS.contains(&key.as_str()) && !value.trim().is_empty() {
            upsert_env_line(&mut lines, &key, value.trim());
            migrated = true;
        }
    }

    if migrated {
        target = join_lines(&lines, target_line_ending);
        write_env_file(target_path, &target)?;
        tracing::info!(
            "Migrated provider configuration from {} to {}",
            legacy_path.display(),
            target_path.display()
        );
    }

    Ok(migrated)
}

fn clear_managed_env_vars() {
    for key in MANAGED_ENV_KEYS {
        env::remove_var(key);
    }
}

fn sanitize_api_key(value: &str) -> Result<String, EnvConfigError> {
    if value.contains('\n') || value.contains('\r') {
        return Err(EnvConfigError::Validation(
            "API key must be a single line.".to_string(),
        ));
    }

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(EnvConfigError::Validation(
            "API key cannot be empty.".to_string(),
        ));
    }

    Ok(trimmed.to_string())
}

fn validate_provider_key(provider: ProviderKind, api_key: &str) -> Result<(), EnvConfigError> {
    match provider {
        ProviderKind::OpenAI => {
            if !api_key.starts_with("sk-") || api_key.len() < 20 {
                return Err(EnvConfigError::Validation(
                    "OpenAI API key should start with 'sk-' and look complete.".to_string(),
                ));
            }
        }
        ProviderKind::Anthropic => {
            if api_key.len() < 16 {
                return Err(EnvConfigError::Validation(
                    "Anthropic API key looks too short.".to_string(),
                ));
            }
        }
        ProviderKind::Gemini => {
            if api_key.len() < 16 {
                return Err(EnvConfigError::Validation(
                    "Gemini API key looks too short.".to_string(),
                ));
            }
        }
        ProviderKind::OpenRouter => {
            if api_key.len() < 16 {
                return Err(EnvConfigError::Validation(
                    "OpenRouter API key looks too short.".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn read_env_file(path: &Path) -> Result<String, EnvConfigError> {
    fs::read_to_string(path).map_err(|err| map_io_error(path, err))
}

fn write_env_file(path: &Path, content: &str) -> Result<(), EnvConfigError> {
    let parent = path.parent().ok_or_else(|| EnvConfigError::Io {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::NotFound, "Missing parent directory"),
    })?;

    fs::create_dir_all(parent).map_err(|err| map_io_error(parent, err))?;

    let temp_name = format!(
        "{}.tmp.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("env"),
        std::process::id()
    );
    let temp_path = parent.join(temp_name);

    if let Err(err) = fs::write(&temp_path, content) {
        return Err(map_io_error(&temp_path, err));
    }

    if let Err(rename_err) = fs::rename(&temp_path, path) {
        if path.exists() {
            if let Err(remove_err) = fs::remove_file(path) {
                let _ = fs::remove_file(&temp_path);
                return Err(map_io_error(path, remove_err));
            }
            if let Err(second_rename_err) = fs::rename(&temp_path, path) {
                let _ = fs::remove_file(&temp_path);
                return Err(map_io_error(path, second_rename_err));
            }
        } else {
            let _ = fs::remove_file(&temp_path);
            return Err(map_io_error(path, rename_err));
        }
    }

    Ok(())
}

fn map_io_error(path: &Path, source: io::Error) -> EnvConfigError {
    if source.kind() == io::ErrorKind::PermissionDenied {
        EnvConfigError::PermissionDenied {
            path: path.to_path_buf(),
            source,
        }
    } else {
        EnvConfigError::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

fn parse_env_entries(content: &str) -> Vec<(String, String)> {
    let mut parsed = Vec::new();

    for raw_line in split_lines(content) {
        if let Some((key, value)) = parse_env_assignment(&raw_line) {
            parsed.push((key.to_string(), strip_optional_quotes(value.trim()).to_string()));
        }
    }

    parsed
}

fn parse_env_assignment(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let equals_index = line.find('=')?;
    let mut key = line[..equals_index].trim();
    if let Some(stripped) = key.strip_prefix("export ") {
        key = stripped.trim();
    }

    if key.is_empty() {
        return None;
    }

    let value = &line[equals_index + 1..];
    Some((key, value))
}

fn strip_optional_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let first = bytes[0] as char;
        let last = bytes[value.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn split_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect()
}

fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else if cfg!(windows) {
        "\r\n"
    } else {
        "\n"
    }
}

fn join_lines(lines: &[String], line_ending: &str) -> String {
    let mut out = lines.join(line_ending);
    out.push_str(line_ending);
    out
}

fn upsert_env_if_missing(lines: &mut Vec<String>, key: &str, value: &str) {
    if find_env_key_line(lines, key).is_none() {
        lines.push(format!("{}={}", key, value));
    }
}

fn upsert_env_line(lines: &mut Vec<String>, key: &str, value: &str) {
    let replacement = format!("{}={}", key, value);
    let mut replaced = false;

    for line in lines.iter_mut() {
        if let Some((line_key, _)) = parse_env_assignment(line) {
            if line_key == key {
                *line = replacement.clone();
                replaced = true;
            }
        }
    }

    if !replaced {
        lines.push(replacement);
    }
}

fn find_env_key_line(lines: &[String], key: &str) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate() {
        if let Some((line_key, _)) = parse_env_assignment(line) {
            if line_key == key {
                return Some(idx);
            }
        }
    }
    None
}

fn debug_enabled() -> bool {
    env::var("BRIDO_DEBUG")
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty() && normalized != "0" && normalized != "false"
        })
        .unwrap_or(false)
}
