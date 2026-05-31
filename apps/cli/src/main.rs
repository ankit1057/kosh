use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use cache_engine::{CacheRecord, ContextCache, ContextFingerprint, ContextLeaseManager};
use cost_estimator::{
    parse_compression_history, summarize_compression, summarize_compression_by_context,
    summarize_compression_by_feature, summarize_compression_by_kind, summarize_compression_by_repo,
    CompressionRecord, CompressionSummary,
};
use indexer::IndexSnapshot;
use mcp_router::{
    default_mcp_aliases, expand_mcp_alias, parse_mcp_aliases, parse_symbol_aliases,
    resolve_symbol_alias, McpAlias, SymbolAlias,
};
use packet_engine::{PacketRecord, PacketStore};
use tool_registry::{default_aliases, expand_command, parse_aliases, CommandAlias};

fn config_dir() -> &'static str {
    if std::path::Path::new(".kosh").exists() {
        ".kosh"
    } else if std::path::Path::new(".rtk").exists() {
        ".rtk"
    } else {
        ".kosh"
    }
}

fn cfg_path(name: &str) -> String {
    format!("{}/{}", config_dir(), name)
}
const BLENDED_COST_PER_TOKEN: f64 = 0.00001;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match run(args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<ExitCode, String> {
    if args.is_empty() {
        print_help();
        return Ok(ExitCode::SUCCESS);
    }

    match args[0].as_str() {
        "--help" | "-h" | "help" => {
            print_help();
            Ok(ExitCode::SUCCESS)
        }
        "--version" | "-V" => {
            println!("kosh {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        "gain" => handle_gain(&args[1..]),
        "index" => handle_index(&args[1..]),
        "config" => handle_config(&args[1..]),
        "expand" => handle_expand(&args[1..]),
        "mcp" => handle_mcp(&args[1..]),
        "cache" => handle_cache(&args[1..]),
        "lease" => handle_lease(&args[1..]),
        "batch" => handle_batch(&args[1..]),
        "packet" => handle_packet(&args[1..]),
        "symbols" => handle_symbols(&args[1..]),
        "proxy" => execute_raw(&args[1..]),
        _ => execute_expanded(&args),
    }
}

fn print_help() {
    println!(
        "kosh <alias|command> [args...]\n\
         \n\
         Examples:\n\
           kosh gs\n\
           kosh config init\n\
           kosh mcp expand \"rf @authrepo\"\n\
           kosh symbols put @authrepo src/auth.rs\n\
           kosh lease create --repo kosh --feature auth --fingerprint xyz --summary \"Auth context\"\n\
           kosh lease touch lease:auth:001\n\
           kosh packet create --name auth --file src/auth.rs --symbol @authrepo\n\
           kosh packet load auth\n\
           kosh batch '[{{\"tool\":\"read_file\",\"path\":\"Cargo.toml\"}}]'\n\
           kosh index\n\
           kosh gain --history"
    );
}

fn handle_index(args: &[String]) -> Result<ExitCode, String> {
    let snapshot = IndexSnapshot::scan(".").map_err(format_io)?;

    match args.first().map(String::as_str) {
        None => {
            print_index_summary(&snapshot);
            Ok(ExitCode::SUCCESS)
        }
        Some("--json") => {
            println!("{}", snapshot.to_compact_json());
            Ok(ExitCode::SUCCESS)
        }
        Some("write") => {
            fs::create_dir_all(config_dir()).map_err(format_io)?;
            fs::write(cfg_path("index.tsv"), snapshot.to_tsv()).map_err(format_io)?;
            print_index_summary(&snapshot);
            Ok(ExitCode::SUCCESS)
        }
        Some("diff") => {
            let previous = load_index_snapshot()?;
            let diff = snapshot.diff(&previous);
            if args.get(1).map(String::as_str) == Some("--json") {
                println!("{}", diff.to_compact_json());
            } else if diff.is_empty() {
                println!("unchanged");
            } else {
                print_index_diff(&diff);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: kosh index [--json|write|diff [--json]]".to_string()),
    }
}

fn print_index_summary(snapshot: &IndexSnapshot) {
    let summary = snapshot.summary();
    println!("files={}", summary.files);
    println!("bytes={}", summary.bytes);
    for (language, count) in summary.by_language {
        println!("language.{language}={count}");
    }
}

fn print_index_diff(diff: &indexer::IndexDiff) {
    for file in &diff.added {
        println!("added\t{}", file.path);
    }
    for file in &diff.modified {
        println!("modified\t{}", file.path);
    }
    for file in &diff.deleted {
        println!("deleted\t{}", file.path);
    }
}

fn handle_gain(args: &[String]) -> Result<ExitCode, String> {
    let records = load_history()?;
    let summary = summarize_compression(&records);

    if args.first().map(String::as_str) == Some("--history") {
        print_history(&records);
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().map(String::as_str) == Some("--json") {
        println!("{}", summary.to_compact_json(BLENDED_COST_PER_TOKEN));
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().map(String::as_str) == Some("--history-json") {
        print_history_json(&records);
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().map(String::as_str) == Some("--by-kind") {
        print_gain_grouped(summarize_compression_by_kind(&records));
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().map(String::as_str) == Some("--by-repo") {
        print_gain_grouped(summarize_compression_by_repo(&records));
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().map(String::as_str) == Some("--by-feature") {
        print_gain_grouped(summarize_compression_by_feature(&records));
        return Ok(ExitCode::SUCCESS);
    }

    if args.first().map(String::as_str) == Some("--by-context") {
        print_gain_grouped(summarize_compression_by_context(&records));
        return Ok(ExitCode::SUCCESS);
    }

    print_gain(summary);
    Ok(ExitCode::SUCCESS)
}

fn print_gain(summary: CompressionSummary) {
    println!("records={}", summary.records);
    println!("failed_records={}", summary.failed_records);
    println!("compact_chars={}", summary.compact_chars);
    println!("expanded_chars={}", summary.expanded_chars);
    println!("saved_chars={}", summary.saved_chars);
    println!("estimated_saved_tokens={}", summary.estimated_saved_tokens);
    println!(
        "estimated_cost_saved=${:.4}",
        summary.estimated_cost_saved(BLENDED_COST_PER_TOKEN)
    );
}

fn print_gain_grouped(groups: Vec<(String, CompressionSummary)>) {
    for (key, summary) in groups {
        println!(
            "{}\trecords={}\tfailed_records={}\tsaved_chars={}\testimated_saved_tokens={}\testimated_cost_saved=${:.4}",
            key,
            summary.records,
            summary.failed_records,
            summary.saved_chars,
            summary.estimated_saved_tokens,
            summary.estimated_cost_saved(BLENDED_COST_PER_TOKEN)
        );
    }
}

fn print_history(records: &[CompressionRecord]) {
    for record in records.iter().rev().take(20).rev() {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\tsaved_chars={}\testimated_saved_tokens={}",
            record.timestamp_seconds,
            record.repo,
            record.feature,
            record.kind,
            record.status,
            record.compact,
            record.expanded,
            record.saved_chars(),
            record.estimated_saved_tokens()
        );
    }
}

fn print_history_json(records: &[CompressionRecord]) {
    print!("[");
    for (index, record) in records.iter().rev().take(20).rev().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!("{}", record.to_compact_json());
    }
    println!("]");
}

fn handle_expand(args: &[String]) -> Result<ExitCode, String> {
    if args.is_empty() {
        return Err("usage: kosh expand <alias|command> [args...]".to_string());
    }

    let aliases = load_command_aliases()?;
    let expanded = expand_command(args, &aliases);
    let compact = args.join(" ");
    let expanded = expanded.join(" ");
    maybe_record_compression("cmd-preview", &compact, &expanded, "preview")?;
    println!("{expanded}");
    Ok(ExitCode::SUCCESS)
}

fn handle_mcp(args: &[String]) -> Result<ExitCode, String> {
    if args.first().map(String::as_str) != Some("expand") {
        return Err("usage: kosh mcp expand \"rf @symbol\"".to_string());
    }

    let input = args[1..].join(" ");
    let aliases = load_mcp_aliases()?;
    let mut expanded = expand_mcp_alias(&input, &aliases)?;
    let symbol_aliases = load_symbol_aliases()?;
    expanded.argument_value = resolve_symbol_alias(&expanded.argument_value, &symbol_aliases);
    let compact = input;
    let expanded_json = expanded.to_compact_json();
    maybe_record_compression("mcp", &compact, &expanded_json, "ok")?;
    println!("{expanded_json}");
    Ok(ExitCode::SUCCESS)
}

fn handle_cache(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("fingerprint") => {
            let fingerprint = fingerprint_from_flags(args)?;
            println!("{}", fingerprint.key());
            println!("{}", fingerprint.to_compact_json());
            Ok(ExitCode::SUCCESS)
        }
        Some("put") => {
            let fingerprint = fingerprint_from_flags(args)?;
            let summary = flag_value(args, "--summary")?;
            let mut cache = ContextCache::load(&cfg_path("cache.tsv")).map_err(format_io)?;
            let record = CacheRecord::new(fingerprint, summary);
            let key = record.key();
            cache.upsert(record);
            cache.save(&cfg_path("cache.tsv")).map_err(format_io)?;
            println!("{key}");
            Ok(ExitCode::SUCCESS)
        }
        Some("get") => {
            let key = args
                .get(1)
                .ok_or_else(|| "usage: kosh cache get <key>".to_string())?;
            let cache = ContextCache::load(&cfg_path("cache.tsv")).map_err(format_io)?;
            let Some(record) = cache.get(key) else {
                return Err(format!("cache miss: {key}"));
            };
            println!("{}", record.to_compact_json());
            Ok(ExitCode::SUCCESS)
        }
        Some("list") => {
            let cache = ContextCache::load(&cfg_path("cache.tsv")).map_err(format_io)?;
            for record in cache.records() {
                println!("{}", record.to_compact_json());
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: kosh cache <fingerprint|put|get|list>".to_string()),
    }
}

fn handle_lease(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("create") => {
            let repo = flag_value(args, "--repo")?;
            let feature = flag_value(args, "--feature")?;
            let fingerprint = flag_value(args, "--fingerprint")?;
            let summary = flag_value(args, "--summary")?;
            let mut manager =
                ContextLeaseManager::load(&cfg_path("leases.tsv")).map_err(format_io)?;
            let lease = manager.create_lease(&repo, &feature, &fingerprint, &summary);
            manager.save(&cfg_path("leases.tsv")).map_err(format_io)?;
            println!("{}", lease.to_compact_json());
            Ok(ExitCode::SUCCESS)
        }
        Some("get") => {
            let id = args
                .get(1)
                .ok_or_else(|| "usage: kosh lease get <id>".to_string())?;
            let manager = ContextLeaseManager::load(&cfg_path("leases.tsv")).map_err(format_io)?;
            let lease = manager.get(id).ok_or_else(|| format!("lease miss: {id}"))?;
            println!("{}", lease.to_compact_json());
            Ok(ExitCode::SUCCESS)
        }
        Some("touch") => {
            let id = args
                .get(1)
                .ok_or_else(|| "usage: kosh lease touch <id>".to_string())?;
            let mut manager =
                ContextLeaseManager::load(&cfg_path("leases.tsv")).map_err(format_io)?;

            // To avoid mutating the lease before formatting its JSON output,
            // we touch it, then print it, then log compression.
            let lease = manager
                .touch(id)
                .ok_or_else(|| format!("lease miss: {id}"))?;
            let lease_json = lease.to_compact_json();

            // Record token compression
            // For now, we simulate the expanded context using a dummy repetition of characters
            // derived from a heuristic (e.g. an assumed 20k chars per lease) to reflect saved context.
            // A more sophisticated system would link the fingerprint to an index map to get true byte size.
            let compact = format!("lease:{}", id);
            let simulated_expanded_chars = 20_000; // heuristic for a typical context lease size
            let expanded_dummy = "a".repeat(simulated_expanded_chars);

            maybe_record_compression("lease_hit", &compact, &expanded_dummy, "ok")?;

            manager.save(&cfg_path("leases.tsv")).map_err(format_io)?;
            println!("{}", lease_json);
            Ok(ExitCode::SUCCESS)
        }
        Some("list") => {
            let manager = ContextLeaseManager::load(&cfg_path("leases.tsv")).map_err(format_io)?;
            for record in manager.records() {
                println!("{}", record.to_compact_json());
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("stats") => {
            let manager = ContextLeaseManager::load(&cfg_path("leases.tsv")).map_err(format_io)?;
            let total_accesses: u64 = manager.records().iter().map(|r| r.access_count).sum();
            println!(
                "{{\"total_leases\":{},\"total_accesses\":{}}}",
                manager.records().len(),
                total_accesses
            );
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: kosh lease <create|get|list|touch|stats>".to_string()),
    }
}

fn handle_config(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("init") => {
            fs::create_dir_all(config_dir()).map_err(format_io)?;
            write_if_missing(cfg_path("commands.aliases"), DEFAULT_COMMAND_ALIAS_CONFIG)?;
            write_if_missing(cfg_path("mcp.aliases"), DEFAULT_MCP_ALIAS_CONFIG)?;
            write_if_missing(cfg_path("symbols.aliases"), DEFAULT_SYMBOL_ALIAS_CONFIG)?;
            println!("created {} config", config_dir());
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: kosh config init".to_string()),
    }
}

fn handle_symbols(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("put") => {
            let symbol = args
                .get(1)
                .ok_or_else(|| "usage: kosh symbols put <@symbol> <value>".to_string())?;
            let value = args
                .get(2)
                .ok_or_else(|| "usage: kosh symbols put <@symbol> <value>".to_string())?;
            let alias = SymbolAlias::new(symbol, value)?;
            let mut aliases = load_symbol_aliases()?;
            aliases.retain(|candidate| candidate.symbol != alias.symbol);
            aliases.push(alias);
            save_symbol_aliases(&aliases)?;
            println!("{symbol}");
            Ok(ExitCode::SUCCESS)
        }
        Some("get") => {
            let symbol = args
                .get(1)
                .ok_or_else(|| "usage: kosh symbols get <@symbol>".to_string())?;
            let aliases = load_symbol_aliases()?;
            let resolved = resolve_symbol_alias(symbol, &aliases);
            if resolved == *symbol {
                return Err(format!("symbol miss: {symbol}"));
            }
            println!("{resolved}");
            Ok(ExitCode::SUCCESS)
        }
        Some("list") => {
            let aliases = load_symbol_aliases()?;
            for alias in aliases {
                println!("{} => {}", alias.symbol, alias.value);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: kosh symbols <put|get|list>".to_string()),
    }
}

fn fingerprint_from_flags(args: &[String]) -> Result<ContextFingerprint, String> {
    let repo = flag_value(args, "--repo")?;
    let feature = flag_value(args, "--feature")?;
    let hash = flag_value(args, "--hash")?;
    Ok(ContextFingerprint::new(repo, feature, hash))
}

fn flag_value(args: &[String], flag: &str) -> Result<String, String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
        .ok_or_else(|| format!("missing {flag}"))
}

fn execute_expanded(args: &[String]) -> Result<ExitCode, String> {
    let aliases = load_command_aliases()?;
    let expanded = expand_command(args, &aliases);
    let compact = args.join(" ");
    let expanded_text = expanded.join(" ");
    let exit_code = execute_raw_code(&expanded)?;
    maybe_record_compression(
        "cmd",
        &compact,
        &expanded_text,
        &format!("exit:{exit_code}"),
    )?;
    Ok(ExitCode::from(exit_code as u8))
}

fn execute_raw(args: &[String]) -> Result<ExitCode, String> {
    let exit_code = execute_raw_code(args)?;
    Ok(ExitCode::from(exit_code as u8))
}

fn execute_raw_code(args: &[String]) -> Result<i32, String> {
    let Some((program, rest)) = args.split_first() else {
        return Err("missing command".to_string());
    };

    let status = Command::new(program)
        .args(rest)
        .status()
        .map_err(|error| format!("failed to execute {program}: {error}"))?;

    Ok(status.code().unwrap_or(1))
}

fn load_command_aliases() -> Result<Vec<CommandAlias>, String> {
    let mut aliases = default_aliases();
    let path = cfg_path("commands.aliases");
    if Path::new(&path).exists() {
        let contents = fs::read_to_string(&path).map_err(format_io)?;
        aliases.extend(parse_aliases(&contents)?);
    }
    Ok(aliases)
}

fn load_mcp_aliases() -> Result<Vec<McpAlias>, String> {
    let mut aliases = default_mcp_aliases();
    let path = cfg_path("mcp.aliases");
    if Path::new(&path).exists() {
        let contents = fs::read_to_string(&path).map_err(format_io)?;
        aliases.extend(parse_mcp_aliases(&contents)?);
    }
    Ok(aliases)
}

fn load_symbol_aliases() -> Result<Vec<SymbolAlias>, String> {
    let path = cfg_path("symbols.aliases");
    if !Path::new(&path).exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(&path).map_err(format_io)?;
    parse_symbol_aliases(&contents)
}

fn save_symbol_aliases(aliases: &[SymbolAlias]) -> Result<(), String> {
    fs::create_dir_all(config_dir()).map_err(format_io)?;
    let mut contents = String::new();
    contents.push_str("# KOSH symbol aliases.\n");
    contents.push_str("# Format: <@symbol> => <value>\n");
    for alias in aliases {
        contents.push_str(&alias.symbol);
        contents.push_str(" => ");
        contents.push_str(&alias.value);
        contents.push('\n');
    }
    fs::write(cfg_path("symbols.aliases"), contents).map_err(format_io)
}

fn write_if_missing(path: impl Into<PathBuf>, contents: &str) -> Result<(), String> {
    let path = path.into();
    if path.exists() {
        return Ok(());
    }

    fs::write(path, contents).map_err(format_io)
}

fn format_io(error: std::io::Error) -> String {
    error.to_string()
}

fn maybe_record_compression(
    kind: &str,
    compact: &str,
    expanded: &str,
    status: &str,
) -> Result<(), String> {
    let record = CompressionRecord::with_metadata(
        current_timestamp_seconds(),
        current_repo_name(),
        current_feature_name(),
        kind,
        compact,
        expanded,
        status,
    );
    if record.saved_chars() == 0 {
        return Ok(());
    }

    fs::create_dir_all(config_dir()).map_err(format_io)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(cfg_path("history.tsv"))
        .map_err(format_io)?;
    file.write_all(record.to_tsv_line().as_bytes())
        .map_err(format_io)
}

fn current_repo_name() -> String {
    env::var("KOSH_REPO")
        .or_else(|_| env::var("RTK_REPO"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            env::current_dir()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "unknown".to_string())
        })
}

fn current_feature_name() -> String {
    env::var("KOSH_FEATURE")
        .or_else(|_| env::var("RTK_FEATURE"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "default".to_string())
}

fn current_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn load_history() -> Result<Vec<CompressionRecord>, String> {
    let path = cfg_path("history.tsv");
    if !Path::new(&path).exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(&path).map_err(format_io)?;
    parse_compression_history(&contents)
}

// ── batch ─────────────────────────────────────────────────────────────────────

/// Extract the string value of a key from a flat JSON object fragment.
/// Handles simple `"key":"value"` pairs where the value contains no embedded
/// unescaped double-quotes (suitable for path/query/tool strings).
fn json_str_field<'a>(obj: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\"", key);
    let mut search_start = 0;
    loop {
        let key_pos = obj[search_start..]
            .find(&needle)
            .map(|p| search_start + p)?;
        // Boundary check: the character before the opening quote must be '{', ',' or whitespace
        // to ensure this is a JSON key and not text inside a value.
        let valid_boundary = if key_pos == 0 {
            true
        } else {
            let prev_byte = obj.as_bytes()[key_pos - 1];
            prev_byte == b'{' || prev_byte == b',' || prev_byte.is_ascii_whitespace()
        };
        if !valid_boundary {
            search_start = key_pos + 1;
            continue;
        }
        let after_key = &obj[key_pos + needle.len()..];
        // skip optional whitespace then colon then optional whitespace then opening quote
        let colon_pos = after_key.find(':')?;
        let after_colon = after_key[colon_pos + 1..].trim_start();
        if !after_colon.starts_with('"') {
            return None;
        }
        let inner = &after_colon[1..];
        // find closing quote, skipping escaped quotes
        let mut end = 0;
        let bytes = inner.as_bytes();
        while end < bytes.len() {
            if bytes[end] == b'\\' {
                end += 2;
            } else if bytes[end] == b'"' {
                return Some(&inner[..end]);
            } else {
                end += 1;
            }
        }
        return None;
    }
}

/// Split a JSON array string into individual object strings.
/// Handles only arrays of flat objects (no nested arrays/objects in values).
fn split_json_objects(array: &str) -> Vec<&str> {
    let array = array.trim();
    if !array.starts_with('[') {
        return vec![];
    }
    // strip surrounding [ ]
    let inner = if array.starts_with('[') && array.ends_with(']') {
        &array[1..array.len() - 1]
    } else {
        array
    };

    let mut objects = Vec::new();
    let mut depth: i32 = 0;
    let mut start: Option<usize> = None;
    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        objects.push(&inner[s..=i]);
                        start = None;
                    }
                }
            }
            b'"' => {
                // skip string literal so braces inside strings are ignored
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        break;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    objects
}

fn parse_batch_calls(input: &str) -> Result<Vec<(String, Option<String>)>, String> {
    let objects = split_json_objects(input);
    if objects.is_empty() {
        return Err("batch: empty or invalid JSON array".to_string());
    }
    let mut calls = Vec::new();
    for obj in objects {
        let tool = json_str_field(obj, "tool")
            .ok_or_else(|| format!("batch: missing \"tool\" key in object: {obj}"))?
            .to_string();
        // Try common argument keys in order of tool convention
        let arg = json_str_field(obj, "path")
            .or_else(|| json_str_field(obj, "query"))
            .map(str::to_string);
        calls.push((tool, arg));
    }
    Ok(calls)
}

fn execute_batch_call(tool: &str, arg: Option<&str>) -> (String, String) {
    match tool {
        "read_file" => {
            let path = match arg {
                Some(p) => p,
                None => return ("err".to_string(), "missing path argument".to_string()),
            };
            match fs::read_to_string(path) {
                Ok(contents) => {
                    const MAX: usize = 4096;
                    let result = if contents.len() > MAX {
                        // Find a valid char boundary at or before MAX to avoid UTF-8 panics
                        let boundary = contents
                            .char_indices()
                            .take_while(|(i, _)| *i < MAX)
                            .last()
                            .map(|(i, c)| i + c.len_utf8())
                            .unwrap_or(MAX.min(contents.len()));
                        format!("{}...[truncated]", &contents[..boundary])
                    } else {
                        contents
                    };
                    ("ok".to_string(), result)
                }
                Err(e) => ("err".to_string(), e.to_string()),
            }
        }
        "list_directory" => {
            let path = match arg {
                Some(p) => p,
                None => return ("err".to_string(), "missing path argument".to_string()),
            };
            match fs::read_dir(path) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect();
                    names.sort();
                    ("ok".to_string(), names.join("\n"))
                }
                Err(e) => ("err".to_string(), e.to_string()),
            }
        }
        "search_files" => {
            let query = match arg {
                Some(q) => q,
                None => return ("err".to_string(), "missing query argument".to_string()),
            };
            if query.starts_with('-') {
                return (
                    "err".to_string(),
                    "search query must not start with '-'".to_string(),
                );
            }
            let output = Command::new("find").args([".", "-name", query]).output();
            match output {
                Ok(out) => {
                    let result = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    ("ok".to_string(), result)
                }
                Err(e) => ("err".to_string(), e.to_string()),
            }
        }
        unknown => ("err".to_string(), format!("unknown tool: {unknown}")),
    }
}

/// Escape a string for inclusion in a JSON string value.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn handle_batch(args: &[String]) -> Result<ExitCode, String> {
    let json_input: String = if args.first().map(String::as_str) == Some("--file") {
        let path = args
            .get(1)
            .ok_or_else(|| "usage: kosh batch --file <path>".to_string())?;
        fs::read_to_string(path).map_err(format_io)?
    } else if args.is_empty() {
        return Err(
            "usage: kosh batch '<json-array>'\n       kosh batch --file <path>".to_string(),
        );
    } else {
        args.join(" ")
    };

    let calls = parse_batch_calls(&json_input)?;

    let mut result_lines: Vec<String> = Vec::new();
    let mut ok_count = 0usize;
    let mut err_count = 0usize;

    for (tool, arg) in &calls {
        let (status, payload) = execute_batch_call(tool, arg.as_deref());
        let line = if status == "ok" {
            ok_count += 1;
            format!(
                "{{\"tool\":\"{}\",\"status\":\"ok\",\"result\":\"{}\"}}",
                json_escape(tool),
                json_escape(&payload)
            )
        } else {
            err_count += 1;
            format!(
                "{{\"tool\":\"{}\",\"status\":\"err\",\"error\":\"{}\"}}",
                json_escape(tool),
                json_escape(&payload)
            )
        };
        println!("{line}");
        result_lines.push(line);
    }

    let overall_status = match (ok_count, err_count) {
        (_, 0) => "ok",
        (0, _) => "err",
        _ => "partial",
    };

    let expanded = result_lines.join("\n");
    maybe_record_compression("mcp_batch", &json_input, &expanded, overall_status)?;

    Ok(ExitCode::SUCCESS)
}

fn load_index_snapshot() -> Result<IndexSnapshot, String> {
    let path = cfg_path("index.tsv");
    if !Path::new(&path).exists() {
        return Ok(IndexSnapshot::default());
    }

    let contents = fs::read_to_string(&path).map_err(format_io)?;
    IndexSnapshot::from_tsv(&contents)
}

const DEFAULT_COMMAND_ALIAS_CONFIG: &str = r#"# KOSH command aliases.
# Format: <alias> => <expansion>
#
# These examples duplicate built-ins so the format is visible.
gs => git status --short
gd => git diff
gl => git log --oneline -20
dart files => find . -name *.dart
"#;

const DEFAULT_MCP_ALIAS_CONFIG: &str = r#"# KOSH MCP aliases.
# Format: <alias> => <tool> <argument_name>
rf => read_file path
sf => search_files query
ls => list_directory path
"#;

const DEFAULT_SYMBOL_ALIAS_CONFIG: &str = r#"# KOSH symbol aliases.
# Format: <@symbol> => <value>
#
# @authrepo => lib/features/auth/data/repositories/auth_repository_impl.dart
"#;

fn handle_packet(args: &[String]) -> Result<ExitCode, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("");

    match subcommand {
        "create" => {
            let mut name: Option<String> = None;
            let mut files: Vec<String> = Vec::new();
            let mut symbols: Vec<String> = Vec::new();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--name" => {
                        i += 1;
                        name = Some(args.get(i).ok_or("--name requires a value")?.clone());
                    }
                    "--file" => {
                        i += 1;
                        files.push(args.get(i).ok_or("--file requires a value")?.clone());
                    }
                    "--symbol" => {
                        i += 1;
                        symbols.push(args.get(i).ok_or("--symbol requires a value")?.clone());
                    }
                    _ => {}
                }
                i += 1;
            }

            let name = name.ok_or(
                "kosh packet create --name <name> [--file <path>]... [--symbol <@sym>]...",
            )?;
            let ts = current_timestamp_seconds();
            let record = PacketRecord::new(&name, files, symbols, ts);
            let json = record.to_compact_json();

            let mut store = PacketStore::load(&cfg_path("packets.tsv")).map_err(format_io)?;
            store.upsert(record);
            store.save(&cfg_path("packets.tsv")).map_err(format_io)?;

            println!("{json}");
            maybe_record_compression("packet_create", &name, &json, "ok")?;
            Ok(ExitCode::SUCCESS)
        }

        "load" => {
            let name = args.get(1).ok_or("kosh packet load <name>")?;

            let store = PacketStore::load(&cfg_path("packets.tsv")).map_err(format_io)?;
            let record = store.get(name).ok_or(format!("packet not found: {name}"))?;

            let mut calls: Vec<String> = Vec::new();
            for path in &record.files {
                calls.push(format!(
                    "{{\"tool\":\"read_file\",\"path\":\"{}\"}}",
                    json_escape(path)
                ));
            }
            for sym in &record.symbols {
                calls.push(format!(
                    "{{\"tool\":\"read_file\",\"path\":\"{}\"}}",
                    json_escape(sym)
                ));
            }

            let batch_json = format!("[{}]", calls.join(","));
            println!("{batch_json}");
            maybe_record_compression("packet_load", name, &batch_json, "ok")?;
            Ok(ExitCode::SUCCESS)
        }

        "list" => {
            let store = PacketStore::load(&cfg_path("packets.tsv")).map_err(format_io)?;
            for record in store.records() {
                println!(
                    "{}\tfiles={}\tsymbols={}",
                    record.name,
                    record.files.len(),
                    record.symbols.len()
                );
            }
            Ok(ExitCode::SUCCESS)
        }

        "delete" => {
            let name = args.get(1).ok_or("kosh packet delete <name>")?;

            let mut store = PacketStore::load(&cfg_path("packets.tsv")).map_err(format_io)?;
            if store.delete(name) {
                store.save(&cfg_path("packets.tsv")).map_err(format_io)?;
                println!("deleted: {name}");
                Ok(ExitCode::SUCCESS)
            } else {
                Err(format!("packet not found: {name}"))
            }
        }

        _ => Err("usage: kosh packet <create|load|list|delete>".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── json_str_field ────────────────────────────────────────────────────────

    #[test]
    fn json_str_field_extracts_simple_value() {
        let obj = r#"{"tool":"read_file","path":"Cargo.toml"}"#;
        assert_eq!(json_str_field(obj, "tool"), Some("read_file"));
        assert_eq!(json_str_field(obj, "path"), Some("Cargo.toml"));
    }

    #[test]
    fn json_str_field_returns_none_for_missing_key() {
        let obj = r#"{"tool":"read_file"}"#;
        assert_eq!(json_str_field(obj, "query"), None);
    }

    #[test]
    fn json_str_field_handles_escaped_quotes_in_value() {
        let obj = r#"{"tool":"read_file","path":"a\"b"}"#;
        assert_eq!(json_str_field(obj, "path"), Some(r#"a\"b"#));
    }

    // ── split_json_objects ────────────────────────────────────────────────────

    #[test]
    fn split_json_objects_two_objects() {
        let input = r#"[{"tool":"read_file","path":"a.txt"},{"tool":"list_directory","path":"."}]"#;
        let objs = split_json_objects(input);
        assert_eq!(objs.len(), 2);
        assert!(objs[0].contains("read_file"));
        assert!(objs[1].contains("list_directory"));
    }

    #[test]
    fn split_json_objects_single_object() {
        let input = r#"[{"tool":"search_files","query":"*.rs"}]"#;
        let objs = split_json_objects(input);
        assert_eq!(objs.len(), 1);
    }

    #[test]
    fn split_json_objects_empty_array() {
        let objs = split_json_objects("[]");
        assert_eq!(objs.len(), 0);
    }

    // ── parse_batch_calls ─────────────────────────────────────────────────────

    #[test]
    fn parse_batch_calls_read_file() {
        let input = r#"[{"tool":"read_file","path":"Cargo.toml"}]"#;
        let calls = parse_batch_calls(input).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "read_file");
        assert_eq!(calls[0].1.as_deref(), Some("Cargo.toml"));
    }

    #[test]
    fn parse_batch_calls_search_files_uses_query_arg() {
        let input = r#"[{"tool":"search_files","query":"*.rs"}]"#;
        let calls = parse_batch_calls(input).unwrap();
        assert_eq!(calls[0].0, "search_files");
        assert_eq!(calls[0].1.as_deref(), Some("*.rs"));
    }

    #[test]
    fn parse_batch_calls_multiple() {
        let input = r#"[{"tool":"read_file","path":"a"},{"tool":"list_directory","path":"b"}]"#;
        let calls = parse_batch_calls(input).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "read_file");
        assert_eq!(calls[1].0, "list_directory");
    }

    #[test]
    fn parse_batch_calls_missing_tool_key_returns_err() {
        let input = r#"[{"path":"a.txt"}]"#;
        assert!(parse_batch_calls(input).is_err());
    }

    // ── execute_batch_call ────────────────────────────────────────────────────

    #[test]
    fn execute_batch_call_unknown_tool_returns_err() {
        let (status, error) = execute_batch_call("unknown_tool", None);
        assert_eq!(status, "err");
        assert!(error.contains("unknown tool: unknown_tool"));
    }

    #[test]
    fn execute_batch_call_read_file_missing_arg_returns_err() {
        let (status, _) = execute_batch_call("read_file", None);
        assert_eq!(status, "err");
    }

    #[test]
    fn execute_batch_call_read_file_truncates_at_4096() {
        use std::io::Write;
        let mut tmp = tempfile_path();
        // write 5000 'x' chars
        let content = "x".repeat(5000);
        fs::write(&tmp, &content).unwrap();
        let (status, result) = execute_batch_call("read_file", Some(&tmp));
        fs::remove_file(&tmp).ok();
        assert_eq!(status, "ok");
        assert!(result.ends_with("...[truncated]"));
        // result should be 4096 chars of content + "...[truncated]" suffix
        assert_eq!(result.len(), 4096 + "...[truncated]".len());
    }

    fn tempfile_path() -> String {
        format!(
            "/tmp/kosh_test_{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )
    }

    // ── json_escape ───────────────────────────────────────────────────────────

    #[test]
    fn json_escape_handles_quotes_and_newlines() {
        assert_eq!(json_escape("say \"hi\"\nbye"), r#"say \"hi\"\nbye"#);
    }

    #[test]
    fn json_escape_passthrough_plain_string() {
        assert_eq!(json_escape("hello world"), "hello world");
    }
}
