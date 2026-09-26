//! `legal-citations`: the JSON API on the command line.
//!
//! Every subcommand is a thin wrapper over `legal_citations::api::call_value`,
//! so the CLI, the Python package and the npm package return identical JSON.

use clap::{Args, Parser, Subcommand, ValueEnum};
use legal_citations::api::{self, ApiError};
use serde_json::{json, Map, Value};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "legal-citations",
    version,
    about = "Find, key, format and link legal citations (JSON out)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Extract citations from each input.
    Extract {
        #[command(flatten)]
        input: Input,
        #[command(flatten)]
        options: ExtractOptions,
        /// Unit of every offset in the output.
        #[arg(long, value_enum, default_value_t = Unit::Char)]
        offset_unit: Unit,
    },
    /// Identity key of every citation in each input.
    Key {
        #[command(flatten)]
        input: Input,
        #[command(flatten)]
        options: ExtractOptions,
    },
    /// Format every citation in each input in a citation style.
    Format {
        #[command(flatten)]
        input: Input,
        #[command(flatten)]
        options: ExtractOptions,
        /// Citation style (mcgill).
        #[arg(long)]
        style: Option<String>,
        /// Output language: en or fr.
        #[arg(long)]
        language: Option<String>,
    },
    /// Public-source URL of every citation in each input.
    Url {
        #[command(flatten)]
        input: Input,
        #[command(flatten)]
        options: ExtractOptions,
        /// Page language: en or fr.
        #[arg(long)]
        language: Option<String>,
        /// Append a single pinpoint's anchor (`#par12`) when the source has one.
        #[arg(long)]
        anchor: bool,
    },
    /// Wrap every citation in markup and print the annotated text.
    Annotate {
        #[command(flatten)]
        input: Input,
        #[command(flatten)]
        options: ExtractOptions,
        /// Inserted before each citation; `{index}`, `{key}`, `{form}`,
        /// `{authority}` and `{url}` are substituted.
        #[arg(long, default_value = "<cite>")]
        before: String,
        /// Inserted after each citation.
        #[arg(long, default_value = "</cite>")]
        after: String,
        /// Wrap the full span (style through parentheticals), not only the core.
        #[arg(long)]
        full_span: bool,
        /// Treat the input as markup: clean it with these steps (repeatable;
        /// e.g. `--clean-step html --clean-step inline_whitespace`), find
        /// citations in the cleaned text and annotate the original markup.
        #[arg(long = "clean-step")]
        clean_steps: Vec<String>,
        /// Print `{"text": ...}` instead of the raw annotated text.
        #[arg(long)]
        json: bool,
    },
    /// Clean text (eyecite-compatible steps: html, inline_whitespace, all_whitespace, underscores, xml).
    Clean {
        #[command(flatten)]
        input: Input,
        #[arg(long = "step", required = true)]
        steps: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Print the embedded registry, or one table of it.
    Registry {
        #[arg(long)]
        table: Option<String>,
        #[arg(long)]
        pretty: bool,
    },
    /// Print crate, schema, key, grammar and registry versions.
    Version {
        #[arg(long)]
        pretty: bool,
    },
    /// Serve many calls in one process: read JSON Lines
    /// `{"id"?, "method", "request"}` from stdin, write one
    /// `{"id", "result"}` or `{"id", "error"}` line per input line.
    Batch,
    /// Call any API method with a JSON request read from a file or stdin.
    Call {
        /// extract, key, keyForText, format, url, annotate, clean, registry,
        /// classifyExcerpt, hasCitation, version.
        method: String,
        /// Request file; stdin when absent or `-`.
        request: Option<PathBuf>,
        #[arg(long)]
        pretty: bool,
    },
}

#[derive(Args)]
struct Input {
    /// Input files; stdin when none or `-`.
    files: Vec<PathBuf>,
    /// Treat each input as JSON Lines: one `{"text": ...}` object or JSON
    /// string per line; output one JSON line per input line.
    #[arg(long)]
    jsonl: bool,
    /// Pretty-print JSON output.
    #[arg(long)]
    pretty: bool,
}

#[derive(Args)]
struct ExtractOptions {
    /// Do not resolve short forms, supra, ibid and references.
    #[arg(long)]
    no_resolve: bool,
    /// Do not group parallel citations.
    #[arg(long)]
    no_parallel: bool,
    /// Skip the extended US reporter/code pass.
    #[arg(long)]
    no_extended_us: bool,
}

impl ExtractOptions {
    fn to_json(&self) -> Value {
        json!({
            "resolve": !self.no_resolve,
            "parallel": !self.no_parallel,
            "extendedUs": !self.no_extended_us,
        })
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Unit {
    Byte,
    Char,
    Utf16,
}

impl Unit {
    fn name(self) -> &'static str {
        match self {
            Self::Byte => "byte",
            Self::Char => "char",
            Self::Utf16 => "utf16",
        }
    }
}

enum Failure {
    Io(String),
    Api(ApiError),
}

impl From<ApiError> for Failure {
    fn from(error: ApiError) -> Self {
        Self::Api(error)
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Io(message)) if message.contains("Broken pipe") => ExitCode::SUCCESS,
        Err(Failure::Io(message)) => {
            eprintln!("{}", json!({"error": {"code": "io", "message": message}}));
            ExitCode::from(1)
        }
        Err(Failure::Api(error)) => {
            eprintln!("{}", json!({"error": error}));
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<(), Failure> {
    match cli.command {
        Command::Extract {
            input,
            options,
            offset_unit,
        } => per_text(
            &input,
            "extract",
            |text| json!({"text": text, "options": options.to_json(), "offsetUnit": offset_unit.name()}),
        ),
        Command::Key { input, options } => per_text(
            &input,
            "keyForText",
            |text| json!({"text": text, "options": options.to_json()}),
        ),
        Command::Format {
            input,
            options,
            style,
            language,
        } => per_text(&input, "format", |text| {
            let mut request = json!({"text": text, "options": options.to_json()});
            if let Some(style) = &style {
                request["style"] = json!(style);
            }
            if let Some(language) = &language {
                request["language"] = json!(language);
            }
            request
        }),
        Command::Url {
            input,
            options,
            language,
            anchor,
        } => per_text(&input, "url", |text| {
            let mut request = json!({"text": text, "options": options.to_json(), "anchor": anchor});
            if let Some(language) = &language {
                request["language"] = json!(language);
            }
            request
        }),
        Command::Annotate {
            input,
            options,
            before,
            after,
            full_span,
            clean_steps,
            json: as_json,
        } => {
            let span = if full_span { "fullSpan" } else { "span" };
            text_output(&input, as_json, "annotate", |source| {
                let mut request = json!({"text": source, "options": options.to_json(), "before": before, "after": after, "span": span});
                if !clean_steps.is_empty() {
                    // A clean failure (unknown step) surfaces from the annotate call itself.
                    let cleaned =
                        api::call_value("clean", json!({"text": source, "steps": clean_steps}))
                            .ok()
                            .and_then(|response| response["text"].as_str().map(str::to_owned))
                            .unwrap_or_else(|| source.to_owned());
                    request["text"] = json!(cleaned);
                    request["source"] = json!(source);
                    request["cleanSteps"] = json!(clean_steps);
                }
                request
            })
        }
        Command::Clean {
            input,
            steps,
            json: as_json,
        } => text_output(
            &input,
            as_json,
            "clean",
            |text| json!({"text": text, "steps": steps}),
        ),
        Command::Registry { table, pretty } => {
            let request = table.map_or_else(|| json!({}), |table| json!({"table": table}));
            emit(&api::call_value("registry", request)?, pretty)
        }
        Command::Batch => batch(),
        Command::Version { pretty } => emit(&api::call_value("version", json!({}))?, pretty),
        Command::Call {
            method,
            request,
            pretty,
        } => {
            let source = read_source(request.as_deref())?;
            let request = if source.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&source)
                    .map_err(|error| Failure::Io(format!("request is not valid JSON: {error}")))?
            };
            emit(&api::call_value(&method, request)?, pretty)
        }
    }
}

fn batch() -> Result<(), Failure> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in io::BufRead::lines(stdin.lock()) {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Err(error) => {
                json!({"id": null, "error": {"code": "invalid_request", "message": format!("line is not JSON: {error}")}})
            }
            Ok(call) => {
                let id = call.get("id").cloned().unwrap_or(Value::Null);
                match call.get("method").and_then(Value::as_str) {
                    None => {
                        json!({"id": id, "error": {"code": "invalid_request", "message": "missing \"method\""}})
                    }
                    Some(method) => {
                        let request = call.get("request").cloned().unwrap_or_else(|| json!({}));
                        match api::call_value(method, request) {
                            Ok(result) => json!({"id": id, "result": result}),
                            Err(error) => json!({"id": id, "error": error}),
                        }
                    }
                }
            }
        };
        writeln!(out, "{response}")?;
        out.flush()?;
    }
    Ok(())
}

/// Texts to process: one per file (or stdin), or one per line in JSONL mode,
/// each labelled with where it came from.
fn texts(input: &Input) -> Result<Vec<(Value, String)>, Failure> {
    let paths = if input.files.is_empty() {
        vec![None]
    } else {
        input
            .files
            .iter()
            .map(|path| (path.as_os_str() != "-").then_some(path.clone()))
            .collect()
    };
    let mut texts = Vec::new();
    for path in paths {
        let label = path
            .as_ref()
            .map_or_else(|| "-".to_owned(), |path| path.display().to_string());
        let source = read_source(path.as_deref())?;
        if !input.jsonl {
            texts.push((json!({"file": label}), source));
            continue;
        }
        for (number, line) in source.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line).map_err(|error| {
                Failure::Io(format!("{label}:{}: not JSON: {error}", number + 1))
            })?;
            let (mut meta, text) = match value {
                Value::String(text) => (Map::new(), text),
                Value::Object(mut object) => match object.remove("text") {
                    Some(Value::String(text)) => (object, text),
                    _ => {
                        return Err(Failure::Io(format!(
                            "{label}:{}: expected a string or an object with a \"text\" string",
                            number + 1
                        )))
                    }
                },
                _ => {
                    return Err(Failure::Io(format!(
                        "{label}:{}: expected a string or an object with a \"text\" string",
                        number + 1
                    )))
                }
            };
            meta.insert("file".into(), json!(label));
            meta.insert("line".into(), json!(number + 1));
            texts.push((Value::Object(meta), text));
        }
    }
    Ok(texts)
}

fn read_source(path: Option<&std::path::Path>) -> Result<String, Failure> {
    match path {
        Some(path) if path.as_os_str() != "-" => std::fs::read_to_string(path)
            .map_err(|error| Failure::Io(format!("{}: {error}", path.display()))),
        _ => {
            let mut source = String::new();
            io::stdin().read_to_string(&mut source)?;
            Ok(source)
        }
    }
}

/// Run `method` on every text. A single plain input prints the bare response;
/// several inputs print an array of `{file, ...response}`; JSONL prints one
/// line per input line.
fn per_text(input: &Input, method: &str, request: impl Fn(&str) -> Value) -> Result<(), Failure> {
    let texts = texts(input)?;
    let single = !input.jsonl && texts.len() == 1;
    let mut results = Vec::new();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for (meta, text) in texts {
        let response = api::call_value(method, request(&text))?;
        if single {
            return emit(&response, input.pretty);
        }
        let mut labelled = meta.as_object().cloned().unwrap_or_default();
        if let Value::Object(fields) = response {
            labelled.extend(fields);
        }
        if input.jsonl {
            writeln!(out, "{}", Value::Object(labelled))?;
        } else {
            results.push(Value::Object(labelled));
        }
    }
    if !input.jsonl {
        drop(out);
        emit(&Value::Array(results), input.pretty)?;
    }
    Ok(())
}

/// Methods whose response is `{text}`: print the text itself unless `--json`.
fn text_output(
    input: &Input,
    as_json: bool,
    method: &str,
    request: impl Fn(&str) -> Value,
) -> Result<(), Failure> {
    if as_json || input.jsonl {
        return per_text(input, method, request);
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for (_, text) in texts(input)? {
        let response = api::call_value(method, request(&text))?;
        out.write_all(response["text"].as_str().unwrap_or_default().as_bytes())?;
    }
    Ok(())
}

fn emit(value: &Value, pretty: bool) -> Result<(), Failure> {
    let rendered = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
    .expect("JSON serializes");
    match writeln!(io::stdout().lock(), "{rendered}") {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}
