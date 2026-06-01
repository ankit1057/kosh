use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use cache_engine::db::{ContextFingerprintV2, DbLeaseRecord, DbLeaseStore};
use cache_engine::{CacheRecord, ContextCache, ContextFingerprint};
use context_resolver::ContextResolver;
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
use context_signatures::{ContextSignature, SignatureStore};
use fact_engine::{FactRecord, FactStore};
use mcp_proxy::{extract_method, is_cacheable, is_notification, ProxyCache, ProxyStats};
use packet_engine::{load_symbol_aliases as load_sym_alias_map, PacketRecord, PacketStore};
use rusqlite::{params, Error as RusqliteError};
use skill_engine::{SkillAction, SkillRecord, SkillStore};
use symbol_extractor::{DartExtractor, RustExtractor, SymbolRecord, SymbolTable};
use tool_registry::{default_aliases, expand_command, parse_aliases, CommandAlias};

fn config_dir() -> &'static str {
    use std::sync::OnceLock;
    static DIR: OnceLock<&'static str> = OnceLock::new();
    DIR.get_or_init(|| {
        if std::path::Path::new(".kosh").exists() {
            ".kosh"
        } else {
            ".rtk"
        }
    })
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
            println!("rtk {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        "gain" => handle_gain(&args[1..]),
        "index" => handle_index(&args[1..]),
        "config" => handle_config(&args[1..]),
        "expand" => handle_expand(&args[1..]),
        "mcp" => handle_mcp(&args[1..]),
        "cache" => handle_cache(&args[1..]),
        "context" => handle_context(&args[1..]),
        "lease" => handle_lease(&args[1..]),
        "batch" => handle_batch(&args[1..]),
        "packet" => handle_packet(&args[1..]),
        "signature" | "sig" => handle_signature(&args[1..]),
        "fact" => handle_fact(&args[1..]),
        "suggest" => handle_suggest(&args[1..]),
        "benchmark" | "bench" => handle_benchmark(&args[1..]),
        "mcp-proxy" => handle_mcp_proxy(&args[1..]),
        "skill" => handle_skill(&args[1..]),
        "symbols" => handle_symbols(&args[1..]),
        "serve" => handle_serve(&args[1..]),
        "proxy" => execute_raw(&args[1..]),
        _ => execute_expanded(&args),
    }
}

fn print_help() {
    println!(
        "rtk <alias|command> [args...]\n\
         \n\
         Examples:\n\
           rtk gs\n\
           rtk config init\n\
           rtk mcp expand \"rf @authrepo\"\n\
           rtk symbols put @authrepo src/auth.rs\n\
           rtk lease create --repo kosh --feature auth --fingerprint xyz --summary \"Auth context\"\n\
           rtk lease touch lease:auth:001\n\
           rtk packet create --name auth --file src/auth.rs --symbol @authrepo\n\
           rtk packet load auth\n\
           rtk batch '[{{\"tool\":\"read_file\",\"path\":\"Cargo.toml\"}}]'\n\
           rtk index\n\
           rtk gain --history"
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
        _ => Err("usage: rtk index [--json|write|diff [--json]]".to_string()),
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
    let items = records
        .iter()
        .map(|r| r.to_compact_json())
        .collect::<Vec<_>>()
        .join(",");
    println!("[{items}]");
}

fn handle_expand(args: &[String]) -> Result<ExitCode, String> {
    let input = args.join(" ");
    let aliases = load_command_aliases()?;
    let expanded = expand_command(args, &aliases).join(" ");

    println!("{expanded}");
    maybe_record_compression("cmd-preview", "ok", &input, &expanded, "ok")?;
    Ok(ExitCode::SUCCESS)
}

fn handle_mcp(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("expand") => {
            let input = args[1..].join(" ");
            let expanded = expand_mcp_alias_with_symbols(&input)?;
            let json = expanded.to_compact_json();
            println!("{json}");
            maybe_record_compression("mcp", "ok", &input, &json, "ok")?;
            Ok(ExitCode::SUCCESS)
        }
        Some("list") => {
            let aliases = load_mcp_aliases()?;
            for alias in aliases {
                println!("{} => {} {}", alias.alias, alias.tool, alias.argument_name);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: rtk mcp <expand|list>".to_string()),
    }
}

fn handle_cache(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("fingerprint") => {
            let repo = flag_value(args, "--repo")?;
            let feature = flag_value(args, "--feature")?;
            let hash = flag_value(args, "--hash")?;
            let fingerprint = ContextFingerprint::new(repo, feature, hash);
            println!("{}", fingerprint.to_compact_json());
            Ok(ExitCode::SUCCESS)
        }
        Some("put") => {
            let repo = flag_value(args, "--repo")?;
            let feature = flag_value(args, "--feature")?;
            let hash = flag_value(args, "--hash")?;
            let summary = flag_value(args, "--summary")?;

            let mut cache = ContextCache::load(cfg_path("cache.tsv")).map_err(format_io)?;
            let fingerprint = ContextFingerprint::new(repo, feature, hash);
            cache.upsert(CacheRecord::new(fingerprint, summary));
            cache.save(cfg_path("cache.tsv")).map_err(format_io)?;
            Ok(ExitCode::SUCCESS)
        }
        Some("get") => {
            let key = args
                .get(1)
                .ok_or_else(|| "usage: rtk cache get <repo:feature:hash>".to_string())?;
            let cache = ContextCache::load(cfg_path("cache.tsv")).map_err(format_io)?;
            let record = cache.get(key).ok_or_else(|| format!("cache miss: {key}"))?;
            println!("{}", record.to_compact_json());
            Ok(ExitCode::SUCCESS)
        }
        Some("list") => {
            let cache = ContextCache::load(cfg_path("cache.tsv")).map_err(format_io)?;
            for record in cache.records() {
                println!("{}", record.to_compact_json());
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: rtk cache <fingerprint|put|get|list>".to_string()),
    }
}

fn handle_context(args: &[String]) -> Result<ExitCode, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("");
    let resolver = ContextResolver::open(cfg_path("kosh.db"), cfg_path("packets.tsv"))?;

    match subcommand {
        "resolve" => {
            let query = args
                .get(1)
                .ok_or_else(|| "usage: rtk context resolve <query>".to_string())?;
            let recommendations = resolver.resolve_query(query);
            for rec in recommendations {
                println!("{}", serde_json::to_string(&rec).unwrap());
            }
            Ok(ExitCode::SUCCESS)
        }
        "suggest" => {
            // Very basic suggestion: just look at current repo fingerprint
            let snapshot = load_index_snapshot()?;
            let mut fingerprint = ContextFingerprintV2::new(
                &current_repo_name(),
                "main", // TODO: detect branch
                "HEAD", // TODO: detect commit
            );
            // Just for demonstration, we could add files from snapshot
            for file in snapshot.files.iter().take(5) {
                fingerprint.add_file(&file.path);
            }

            if let Some(rec) = resolver.resolve_from_fingerprint(&fingerprint) {
                println!("{}", serde_json::to_string(&rec).unwrap());
            } else {
                println!("{{\"suggestion\":\"None\",\"reason\":\"No matching lease found for current state.\"}}");
            }
            Ok(ExitCode::SUCCESS)
        }
        "explain" => {
            println!("Context Resolution ROI Logic:");
            println!("1. Fingerprint Match: 100% confidence, saves full context retransmission.");
            println!("2. Packet Match: 95% confidence, saves multiple discovery turns.");
            println!("3. Keyword Match: 70% confidence, aids discovery.");
            Ok(ExitCode::SUCCESS)
        }
        "signature" => {
            let lease_id = args
                .get(1)
                .ok_or_else(|| "usage: rtk context signature <lease_id>".to_string())?;
            let table = SymbolTable::open(cfg_path("kosh.db")).map_err(format_db_error)?;
            let symbols = table.get_lease_signature(lease_id).map_err(format_db_error)?;
            let meta = table.get_signature_metadata(lease_id).map_err(format_db_error)?;

            println!("Lease:");
            println!("{lease_id}");

            println!("\nSymbols:");
            for sym in &symbols {
                println!("- {sym}");
            }

            if let Some((hash, version, count)) = meta {
                println!("\nSignature Hash:");
                println!("{hash}");
                println!("\nVersion:");
                println!("{version}");
                println!("\nSymbol Count:");
                println!("{count}");
            } else {
                println!("\nSymbol Count:");
                println!("{}", symbols.len());
            }

            Ok(ExitCode::SUCCESS)
        }
        "extract" => {
            let lease_id = args
                .get(1)
                .ok_or_else(|| "usage: rtk context extract <lease_id>".to_string())?;
            let lease_store = DbLeaseStore::open(cfg_path("kosh.db")).map_err(format_db_error)?;
            let symbol_table = SymbolTable::open(cfg_path("kosh.db")).map_err(format_db_error)?;

            let leases = lease_store.list_all().map_err(format_db_error)?;
            let lease = leases
                .iter()
                .find(|l| l.id == *lease_id)
                .ok_or_else(|| format!("lease not found: {lease_id}"))?;

            let mut all_symbols = Vec::new();
            for file_path in &lease.file_list {
                if let Ok(source) = fs::read_to_string(file_path) {
                    let symbols = if file_path.ends_with(".dart") {
                        DartExtractor::extract_symbols(&source)
                    } else if file_path.ends_with(".rs") {
                        RustExtractor::extract_symbols(&source)
                    } else {
                        Vec::new()
                    };
                    for (name, kind) in symbols {
                        let record = SymbolRecord {
                            id: None,
                            name: name.clone(),
                            kind,
                            file_path: file_path.clone(),
                            repo: lease.repo.clone(),
                            content_hash: "TODO".to_string(),
                        };
                        let sym_id = symbol_table.insert_symbol(&record).map_err(format_db_error)?;
                        symbol_table
                            .associate_lease(&lease.id, sym_id)
                            .map_err(format_db_error)?;
                        all_symbols.push(name);
                    }
                }
            }

            let hash = symbol_extractor::compute_signature_hash(&all_symbols);
            symbol_table
                .upsert_signature(&lease.id, &hash, all_symbols.len())
                .map_err(format_db_error)?;

            println!(
                "Extracted {} symbols for lease {}. Signature: {}",
                all_symbols.len(),
                lease_id,
                hash
            );
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: rtk context <resolve|suggest|explain|signature|extract>".to_string()),
    }
}

fn handle_lease(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("create") => {
            let mut repo: Option<String> = None;
            let mut feature: Option<String> = None;
            let mut fingerprint_hash: Option<String> = None;
            let mut summary: Option<String> = None;
            let mut byte_size: Option<u64> = None;
            let mut files: Vec<String> = Vec::new();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--repo" => {
                        i += 1;
                        repo = Some(args.get(i).ok_or("--repo requires value")?.clone());
                    }
                    "--feature" => {
                        i += 1;
                        feature = Some(args.get(i).ok_or("--feature requires value")?.clone());
                    }
                    "--fingerprint" => {
                        i += 1;
                        fingerprint_hash =
                            Some(args.get(i).ok_or("--fingerprint requires value")?.clone());
                    }
                    "--summary" => {
                        i += 1;
                        summary = Some(args.get(i).ok_or("--summary requires value")?.clone());
                    }
                    "--size" => {
                        i += 1;
                        byte_size = Some(
                            args.get(i)
                                .ok_or("--size requires value")?
                                .parse()
                                .map_err(|_| "invalid size")?,
                        );
                    }
                    "--file" => {
                        i += 1;
                        files.push(args.get(i).ok_or("--file requires value")?.clone());
                    }
                    _ => {}
                }
                i += 1;
            }

            let repo = repo.ok_or("missing --repo")?;
            let feature = feature.ok_or("missing --feature")?;
            let fingerprint_hash = fingerprint_hash.ok_or("missing --fingerprint")?;
            let summary = summary.unwrap_or_default();
            let byte_size = byte_size.unwrap_or_else(|| {
                load_index_snapshot()
                    .map(|snapshot| snapshot.summary().bytes)
                    .unwrap_or(20_000)
            });

            let store = DbLeaseStore::open(cfg_path("kosh.db")).map_err(format_db_error)?;
            // Generate a simple ID for now
            let id = format!(
                "lease:{}:{:03}",
                feature,
                current_timestamp_seconds() % 1000
            );

            let record = DbLeaseRecord {
                id: id.clone(),
                repo,
                feature,
                fingerprint_hash,
                summary,
                byte_size,
                created_at: current_timestamp_seconds(),
                access_count: 0,
                last_used: None,
                tokens_saved: 0,
                file_list: files,
            };

            store.insert_lease(&record).map_err(format_db_error)?;
            println!("{{\"id\":\"{}\"}}", id);
            Ok(ExitCode::SUCCESS)
        }
        Some("get") => {
            let id = args
                .get(1)
                .ok_or_else(|| "usage: rtk lease get <id>".to_string())?;
            let store = DbLeaseStore::open(cfg_path("kosh.db")).map_err(format_db_error)?;
            let mut stmt = store
                .conn
                .prepare("SELECT id, repo, feature, fingerprint_hash, summary, byte_size, created_at, access_count, last_used, tokens_saved FROM leases WHERE id = ?")
                .map_err(format_db_error)?;
            let record = stmt
                .query_row(params![id], |row: &rusqlite::Row| {
                    Ok(format!(
                        "{{\"id\":\"{}\",\"repo\":\"{}\",\"feature\":\"{}\",\"fingerprint\":\"{}\",\"summary\":\"{}\",\"byte_size\":{},\"created_at\":{},\"access_count\":{},\"last_used\":{:?},\"tokens_saved\":{}}}",
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4).unwrap_or_default(),
                        row.get::<_, u64>(5)?,
                        row.get::<_, u64>(6)?,
                        row.get::<_, u64>(7)?,
                        row.get::<_, Option<u64>>(8)?,
                        row.get::<_, u64>(9)?,
                    ))
                })
                .map_err(|_| format!("lease miss: {id}"))?;
            println!("{}", record);
            Ok(ExitCode::SUCCESS)
        }
        Some("list") => {
            let store = DbLeaseStore::open(cfg_path("kosh.db")).map_err(format_db_error)?;
            let leases = store.list_all().map_err(format_db_error)?;
            for lease in leases {
                println!(
                    "{{\"id\":\"{}\",\"repo\":\"{}\",\"feature\":\"{}\",\"byte_size\":{},\"access_count\":{}}}",
                    lease.id, lease.repo, lease.feature, lease.byte_size, lease.access_count
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("stats") => {
            let store = DbLeaseStore::open(cfg_path("kosh.db")).map_err(format_db_error)?;
            let leases = store.list_all().map_err(format_db_error)?;
            let total_accesses: u64 = leases.iter().map(|r| r.access_count).sum();
            let total_tokens: u64 = leases.iter().map(|r| r.tokens_saved).sum();
            println!(
                "{{\"total_leases\":{},\"total_accesses\":{},\"total_tokens_saved\":{}}}",
                leases.len(),
                total_accesses,
                total_tokens
            );
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: rtk lease <create|get|list|touch|stats>".to_string()),
    }
}

fn touch_lease_logic(id: &str) -> Result<String, String> {
    let store = DbLeaseStore::open(cfg_path("kosh.db")).map_err(format_db_error)?;
    let mut stmt = store
        .conn
        .prepare("SELECT byte_size FROM leases WHERE id = ?")
        .map_err(format_db_error)?;
    let byte_size: u64 = stmt
        .query_row(params![id], |r: &rusqlite::Row| r.get(0))
        .map_err(|_| format!("lease miss: {id}"))?;

    // In a real system, we'd estimate tokens based on actual content
    let simulated_tokens = byte_size / 4;
    store
        .record_hit(id, simulated_tokens)
        .map_err(format_db_error)?;

    let compact = format!("lease:{}", id);
    let expanded_dummy = "a".repeat(byte_size as usize);
    maybe_record_compression("lease_hit", "ok", &compact, &expanded_dummy, "ok")?;

    Ok(format!("{{\"id\":\"{}\",\"byte_size\":{}}}", id, byte_size))
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
        _ => Err("usage: rtk config init".to_string()),
    }
}

fn handle_symbols(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("put") => {
            let symbol = args
                .get(1)
                .ok_or_else(|| "usage: rtk symbols put <@symbol> <value>".to_string())?;
            let value = args
                .get(2)
                .ok_or_else(|| "usage: rtk symbols put <@symbol> <value>".to_string())?;
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
                .ok_or_else(|| "usage: rtk symbols get <@symbol>".to_string())?;
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
                println!("{}\t{}", alias.symbol, alias.value);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Err("usage: rtk symbols <put|get|list>".to_string()),
    }
}

fn flag_value(args: &[String], flag: &str) -> Result<String, String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .map(|value| value.to_string())
        .ok_or_else(|| format!("missing {flag}"))
}

fn execute_expanded(args: &[String]) -> Result<ExitCode, String> {
    let input = args.join(" ");
    let aliases = load_command_aliases()?;
    let expanded = expand_command(args, &aliases).join(" ");

    let parts: Vec<String> = expanded.split_whitespace().map(|s| s.to_string()).collect();
    let exit_code = execute_raw_code(&parts)?;

    maybe_record_compression(
        "cmd",
        "ok",
        &input,
        &expanded,
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
    contents.push_str("# RTK symbol aliases.\n");
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

fn format_db_error(error: RusqliteError) -> String {
    error.to_string()
}

fn maybe_record_compression(
    kind: &str,
    status: &str,
    compact: &str,
    expanded: &str,
    _meta: &str,
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
    env::var("RTK_REPO")
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
    env::var("RTK_FEATURE")
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
            .ok_or_else(|| "usage: rtk batch --file <path>".to_string())?;
        fs::read_to_string(path).map_err(format_io)?
    } else if args.is_empty() {
        return Err(
            "usage: rtk batch '<json-array>'\n       rtk batch --file <path>".to_string(),
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
    maybe_record_compression("mcp_batch", "ok", &json_input, &expanded, overall_status)?;

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

const DEFAULT_COMMAND_ALIAS_CONFIG: &str = r#"# RTK command aliases.
# Format: <alias> => <expansion>
#
# These examples duplicate built-ins so the format is visible.
gs => git status --short
gd => git diff
gl => git log --oneline -20
dart files => find . -name *.dart
"#;

const DEFAULT_MCP_ALIAS_CONFIG: &str = r#"# RTK MCP aliases.
# Format: <alias> => <tool> <argument_name>
rf => read_file path
sf => search_files query
ls => list_directory path
"#;

const DEFAULT_SYMBOL_ALIAS_CONFIG: &str = r#"# RTK symbol aliases.
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
                "rtk packet create --name <name> [--file <path>]... [--symbol <@sym>]...",
            )?;
            let ts = current_timestamp_seconds();
            let record = PacketRecord::new(&name, files, symbols, ts);
            let json = record.to_compact_json();

            let mut store = PacketStore::load(&cfg_path("packets.tsv")).map_err(format_io)?;
            store.upsert(record);
            store.save(&cfg_path("packets.tsv")).map_err(format_io)?;

            println!("{json}");
            maybe_record_compression("packet_create", "ok", &name, &json, "ok")?;
            Ok(ExitCode::SUCCESS)
        }

        "load" => {
            let name = args
                .get(1)
                .ok_or_else(|| "usage: rtk packet load <name>".to_string())?;
            let json = load_packet_as_batch(name)?;
            println!("{}", json);
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
            let name = args.get(1).ok_or("rtk packet delete <name>")?;

            let mut store = PacketStore::load(&cfg_path("packets.tsv")).map_err(format_io)?;
            if store.delete(name) {
                store.save(&cfg_path("packets.tsv")).map_err(format_io)?;
                println!("deleted: {name}");
                Ok(ExitCode::SUCCESS)
            } else {
                Err(format!("packet not found: {name}"))
            }
        }

        _ => Err("usage: rtk packet <create|load|list|delete>".to_string()),
    }
}

fn load_packet_as_batch(name: &str) -> Result<String, String> {
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
        let resolved = resolve_symbol_alias(sym, &load_symbol_aliases()?);
        calls.push(format!(
            "{{\"tool\":\"read_file\",\"path\":\"{}\"}}",
            json_escape(&resolved)
        ));
    }

    let batch_json = format!("[{}]", calls.join(","));
    maybe_record_compression("packet_load", "ok", name, &batch_json, "ok")?;
    Ok(batch_json)
}

fn handle_skill(args: &[String]) -> Result<ExitCode, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("");

    match subcommand {
        "create" => {
            let mut name: Option<String> = None;
            let mut description: Option<String> = None;
            let mut actions: Vec<SkillAction> = Vec::new();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--name" => {
                        i += 1;
                        name = Some(args.get(i).ok_or("--name requires a value")?.clone());
                    }
                    "--desc" => {
                        i += 1;
                        description = Some(args.get(i).ok_or("--desc requires a value")?.clone());
                    }
                    "--cmd" => {
                        i += 1;
                        actions.push(SkillAction::new(
                            "cmd",
                            args.get(i).ok_or("--cmd requires a value")?.clone(),
                        ));
                    }
                    "--mcp" => {
                        i += 1;
                        actions.push(SkillAction::new(
                            "mcp",
                            args.get(i).ok_or("--mcp requires a value")?.clone(),
                        ));
                    }
                    _ => {}
                }
                i += 1;
            }

            let name = name.ok_or("rtk skill create --name <name> --desc <description> [--cmd <cmd>]... [--mcp <mcp>]...")?;
            let description = description.unwrap_or_default();

            let mut store = SkillStore::load(&cfg_path("skills.tsv")).map_err(format_io)?;
            let record = SkillRecord::new(name.clone(), description, actions);
            let json = record.to_compact_json();
            store.upsert(record);
            store.save(&cfg_path("skills.tsv")).map_err(format_io)?;

            println!("{json}");
            maybe_record_compression("skill_create", "ok", &name, &json, "ok")?;
            Ok(ExitCode::SUCCESS)
        }

        "run" => {
            let name = args
                .get(1)
                .ok_or_else(|| "usage: rtk skill run <name>".to_string())?;
            let report = run_skill_logic(name)?;
            println!("{}", report);
            Ok(ExitCode::SUCCESS)
        }

        "list" => {
            let store = SkillStore::load(&cfg_path("skills.tsv")).map_err(format_io)?;
            for record in store.records() {
                println!(
                    "{}\tdesc={}\tactions={}",
                    record.name,
                    record.description,
                    record.actions.len()
                );
            }
            Ok(ExitCode::SUCCESS)
        }

        _ => Err("usage: rtk skill <create|run|list>".to_string()),
    }
}

fn run_skill_logic(name: &str) -> Result<String, String> {
    let store = SkillStore::load(&cfg_path("skills.tsv")).map_err(format_io)?;
    let record = store.get(name).ok_or(format!("skill not found: {name}"))?;

    let mut expanded_lines = Vec::new();
    for action in &record.actions {
        match action.kind.as_str() {
            "cmd" => {
                println!("> {}", action.value);
                let parts: Vec<String> = action
                    .value
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                execute_expanded(&parts)?;
                expanded_lines.push(action.value.clone());
            }
            "mcp" => {
                let expanded = expand_mcp_alias_with_symbols(&action.value)?;
                let json = expanded.to_compact_json();
                expanded_lines.push(json);
            }
            _ => {}
        }
    }

    let expanded = expanded_lines.join("\n");
    maybe_record_compression("skill_run", "ok", name, &expanded, "ok")?;
    Ok(expanded)
}
// ── Signature ─────────────────────────────────────────────────────────────────

const SIGNATURES_FILE: &str = "signatures.tsv";

fn handle_signature(args: &[String]) -> Result<ExitCode, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("help");

    match subcommand {
        "create" => {
            println!("STUB: `kosh signature create` called.");
            println!("STUB: Args received: {:?}", &args[1..]);
            // In full implementation, parse --repo, --feature, --file, --symbol
            // and create a new ContextSignature, then save to store.
            Ok(ExitCode::SUCCESS)
        }
        "list" => {
            println!("STUB: `kosh signature list` called.");
            // In full implementation, load store and print all signatures.
            Ok(ExitCode::SUCCESS)
        }
        "match" => {
            println!("STUB: `kosh signature match` called.");
            println!("STUB: Args received: {:?}", &args[1..]);
            // In full implementation, parse --file, --threshold and
            // build a query signature to find matches.
            Ok(ExitCode::SUCCESS)
        }
        "delete" => {
            println!("STUB: `kosh signature delete` called.");
            println!("STUB: Args received: {:?}", &args[1..]);
            // In full implementation, parse ID and delete from store.
            Ok(ExitCode::SUCCESS)
        }
        "touch" => {
            println!("STUB: `kosh signature touch` called.");
            println!("STUB: Args received: {:?}", &args[1..]);
            // In full implementation, parse ID and increment hit_count.
            Ok(ExitCode::SUCCESS)
        }
        "help" | _ => {
            println!("usage: kosh signature <create|list|match|delete|touch|help>");
            println!("\nSubcommands:");
            println!("  create: Create a new context signature from files and symbols.");
            println!("    --repo <repo> --feature <feat> --file <path>... [--symbol <sym>...]");
            println!("  list: List all stored context signatures.");
            println!("  match: Find signatures that overlap with a set of files.");
            println!("    --file <path>... [--threshold 0.3]");
            println!("  delete: Delete a signature by its ID.");
            println!("    <id>");
            println!("  touch: Increment the hit counter for a signature by its ID.");
            println!("    <id>");
            Ok(ExitCode::SUCCESS)
        }
    }
}

// ── Fact ──────────────────────────────────────────────────────────────────────

const FACTS_FILE: &str = "facts.tsv";

fn handle_fact(args: &[String]) -> Result<ExitCode, String> {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let path = std::path::PathBuf::from(cfg_path(FACTS_FILE));

    match sub {
        "create" => {
            let mut repo     = current_repo_name();
            let mut feature  = current_feature_name();
            let mut fact     = String::new();
            let mut confidence: f32 = 0.9;
            let mut source   = String::from("manual");
            let mut symbols: Vec<String> = Vec::new();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--repo"       => { i += 1; if i < args.len() { repo = args[i].clone(); } }
                    "--feature"    => { i += 1; if i < args.len() { feature = args[i].clone(); } }
                    "--fact"       => { i += 1; if i < args.len() { fact = args[i].clone(); } }
                    "--confidence" => { i += 1; if i < args.len() { confidence = args[i].parse().unwrap_or(0.9); } }
                    "--source"     => { i += 1; if i < args.len() { source = args[i].clone(); } }
                    "--symbol"     => { i += 1; if i < args.len() { symbols.push(args[i].clone()); } }
                    _ => { if fact.is_empty() { fact = args[i].clone(); } }
                }
                i += 1;
            }
            if fact.is_empty() {
                return Err("usage: kosh fact create --fact \"<text>\" [--confidence 0.9] [--source <src>] [--symbol @sym]".into());
            }
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            let mut store = FactStore::load(&path).map_err(|e| e.to_string())?;
            let counter = store.records().len() + 1;
            let id = FactStore::make_id(&repo, &feature, counter);
            let record = FactRecord { id: id.clone(), repo, feature, fact, confidence, source, symbols, created_at: now, access_count: 0 };
            store.upsert(record);
            store.save(&path).map_err(|e| e.to_string())?;
            println!("{id}");
            Ok(ExitCode::SUCCESS)
        }

        "list" => {
            let store = FactStore::load(&path).map_err(|e| e.to_string())?;
            for r in store.records() {
                println!("{}\t{:.2}\t{}\t{}", r.id, r.confidence, r.source, &r.fact[..r.fact.len().min(80)]);
            }
            Ok(ExitCode::SUCCESS)
        }

        "get" => {
            let id = args.get(1).ok_or("usage: kosh fact get <id>")?;
            let store = FactStore::load(&path).map_err(|e| e.to_string())?;
            match store.get(id) {
                Some(r) => {
                    println!("id:         {}", r.id);
                    println!("fact:       {}", r.fact);
                    println!("confidence: {:.2}", r.confidence);
                    println!("source:     {}", r.source);
                    println!("symbols:    {}", r.symbols.join(", "));
                    println!("repo:       {}", r.repo);
                    println!("feature:    {}", r.feature);
                    println!("accessed:   {}", r.access_count);
                }
                None => { eprintln!("not found: {id}"); return Ok(ExitCode::from(1)); }
            }
            Ok(ExitCode::SUCCESS)
        }

        "search" => {
            let query = args.get(1).ok_or("usage: kosh fact search <query>")?;
            let store = FactStore::load(&path).map_err(|e| e.to_string())?;
            let hits = store.search(query);
            if hits.is_empty() { println!("no results"); }
            for r in hits {
                println!("{}\t{:.2}\t{}", r.id, r.confidence, &r.fact[..r.fact.len().min(100)]);
            }
            Ok(ExitCode::SUCCESS)
        }

        "touch" => {
            let id = args.get(1).ok_or("usage: kosh fact touch <id>")?;
            let mut store = FactStore::load(&path).map_err(|e| e.to_string())?;
            if store.touch(id) { store.save(&path).map_err(|e| e.to_string())?; println!("ok"); }
            else { eprintln!("not found: {id}"); return Ok(ExitCode::from(1)); }
            Ok(ExitCode::SUCCESS)
        }

        "delete" => {
            let id = args.get(1).ok_or("usage: kosh fact delete <id>")?;
            let mut store = FactStore::load(&path).map_err(|e| e.to_string())?;
            if store.delete(id) { store.save(&path).map_err(|e| e.to_string())?; println!("deleted {id}"); }
            else { eprintln!("not found: {id}"); return Ok(ExitCode::from(1)); }
            Ok(ExitCode::SUCCESS)
        }

        _ => {
            eprintln!("usage: kosh fact <create|list|get|search|touch|delete>");
            Ok(ExitCode::from(1))
        }
    }
}

// ── Suggest ───────────────────────────────────────────────────────────────────

fn handle_suggest(args: &[String]) -> Result<ExitCode, String> {
    // kosh suggest --file <path> [--file <path>...] [--threshold 0.2]
    // Finds signatures overlapping with the given files, then emits the
    // packet load commands that would satisfy them most efficiently.
    let mut files: Vec<String> = Vec::new();
    let mut threshold: f32 = 0.2;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--file"      => { i += 1; if i < args.len() { files.push(args[i].clone()); } }
            "--threshold" => { i += 1; if i < args.len() { threshold = args[i].parse().unwrap_or(0.2); } }
            _ => {}
        }
        i += 1;
    }

    if files.is_empty() {
        return Err("usage: kosh suggest --file <path> [--file <path>...] [--threshold 0.2]".into());
    }

    let now     = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let query   = ContextSignature::new(current_repo_name(), "query", files.clone(), vec![], now);
    let sig_path = std::path::PathBuf::from(cfg_path(SIGNATURES_FILE));
    let sig_store = SignatureStore::load(&sig_path).map_err(|e| e.to_string())?;
    let matches   = sig_store.find_overlapping(&query, threshold);

    let pkt_path  = std::path::PathBuf::from(cfg_path("packets.tsv"));
    let pkt_store = PacketStore::load(&pkt_path).map_err(|e| e.to_string())?;

    let fact_path = std::path::PathBuf::from(cfg_path(FACTS_FILE));
    let fact_store = FactStore::load(&fact_path).map_err(|e| e.to_string())?;

    if matches.is_empty() && pkt_store.records().is_empty() {
        println!("# No matching context found. Consider creating a packet:");
        for f in &files { println!("#   kosh packet create --name <name> --file {f}"); }
        return Ok(ExitCode::SUCCESS);
    }

    println!("# Context suggestions for {} file(s):", files.len());
    println!();

    // Suggest packets whose files overlap with query
    let sym_aliases = load_sym_alias_map(std::path::Path::new(&cfg_path("symbols.aliases")))
        .unwrap_or_default();
    for record in pkt_store.records() {
        let (resolved, _) = PacketStore::resolve_symbols(record, &sym_aliases);
        let overlap: Vec<_> = files.iter().filter(|f| resolved.contains(f)).collect();
        if !overlap.is_empty() {
            println!("kosh packet load {}  # covers {}/{} requested files",
                record.name, overlap.len(), files.len());
        }
    }

    // Suggest matching signatures
    for (sig, score) in &matches {
        println!("kosh signature touch {}  # overlap {:.0}% ({} files)",
            sig.id, score * 100.0, sig.files.len());
    }

    // Suggest related facts
    let related_facts: Vec<_> = files.iter()
        .flat_map(|f| fact_store.search(f))
        .collect();
    if !related_facts.is_empty() {
        println!();
        println!("# Related facts:");
        for fact in related_facts.iter().take(5) {
            println!("#   [{:.2}] {} ({})", fact.confidence, &fact.fact[..fact.fact.len().min(80)], fact.source);
        }
    }

    Ok(ExitCode::SUCCESS)
}

// ── Benchmark ─────────────────────────────────────────────────────────────────

fn handle_benchmark(args: &[String]) -> Result<ExitCode, String> {
    // kosh benchmark [--json] [--suite compression|leasing|packet|batching]
    let json_out = args.iter().any(|a| a == "--json");
    let suite_filter = args.iter().position(|a| a == "--suite")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str);

    let script = std::path::Path::new("agent_test/kosh_benchmarks.py");
    if !script.exists() {
        return Err("benchmark script not found: agent_test/kosh_benchmarks.py".into());
    }

    let mut cmd = Command::new("python3");
    cmd.arg(script);
    if let Some(suite) = suite_filter {
        cmd.arg("--suite").arg(suite);
    }

    let out = cmd.output().map_err(|e| format!("failed to run benchmark: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    if json_out {
        // Extract summary numbers from the Python output and emit JSON
        let mut naive_tok = 0u64;
        let mut kosh_tok  = 0u64;
        let mut saved_tok = 0u64;
        let mut saved_pct = 0.0f64;
        let mut calls_saved = 0i64;
        for line in stdout.lines() {
            if line.contains("TOTAL") {
                let nums: Vec<&str> = line.split_whitespace().collect();
                if nums.len() >= 5 {
                    naive_tok   = nums[1].replace(',', "").parse().unwrap_or(0);
                    kosh_tok    = nums[2].replace(',', "").parse().unwrap_or(0);
                    let saved_s = nums[3].replace([',', '+'], "");
                    saved_tok   = saved_s.parse().unwrap_or(0);
                    let pct_s   = nums[4].replace(['%', '+'], "");
                    saved_pct   = pct_s.parse().unwrap_or(0.0);
                    calls_saved = nums.last().and_then(|s| s.replace('+', "").parse().ok()).unwrap_or(0);
                }
            }
        }
        println!("{{\"naive_tokens\":{naive_tok},\"kosh_tokens\":{kosh_tok},\
            \"saved_tokens\":{saved_tok},\"saved_pct\":{saved_pct:.1},\
            \"calls_saved\":{calls_saved}}}");
    } else {
        print!("{stdout}");
        if !stderr.is_empty() { eprint!("{stderr}"); }
    }

    if out.status.success() { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::from(1)) }
}

// ── MCP Proxy ─────────────────────────────────────────────────────────────────

fn handle_mcp_proxy(_args: &[String]) -> Result<ExitCode, String> {
    use std::io::{self, BufRead};

    let stdin  = io::stdin();
    let stdout = io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    let mut cache = ProxyCache::new();
    let mut stats = ProxyStats::new();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.is_empty() { continue; }

        // Pass notifications through unchanged
        if is_notification(&line) {
            writeln!(out, "{line}").ok();
            continue;
        }

        let method = extract_method(&line).unwrap_or_default();

        if is_cacheable(&method) {
            let key = mcp_proxy::CacheKey::from_message(&method, &line);
            if let Some(cached) = cache.get(&key) {
                // Cache hit — return stored response
                stats.record_hit(cached.byte_size);
                writeln!(out, "{}", cached.response).ok();
                continue;
            }
            // Cache miss — forward to stdout for upstream, record for next time
            // In real proxy mode we'd pipe to the upstream server process.
            // Here we emit the request and expect the caller to feed back the response.
            stats.record_miss();
            writeln!(out, "{line}").ok();
        } else {
            // Non-cacheable — pass through
            writeln!(out, "{line}").ok();
        }
    }

    // Emit stats to stderr so they don't pollute the JSON-RPC stream
    eprintln!("kosh mcp-proxy: hits={} misses={} rate={:.1}% bytes_avoided={}",
        stats.cache_hits, stats.requests_intercepted - stats.cache_hits,
        stats.hit_rate() * 100.0, stats.bytes_avoided);

    Ok(ExitCode::SUCCESS)
}

fn handle_serve(_args: &[String]) -> Result<ExitCode, String> {
    use std::io::{self, BufRead};

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.map_err(format_io)?;
        if line.trim().is_empty() {
            continue;
        }

        let id = json_str_field(&line, "id")
            .or_else(|| json_num_field(&line, "id"))
            .unwrap_or("null");

        if line.contains("\"method\":\"initialize\"") {
            println!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{}},\"serverInfo\":{{\"name\":\"kosh\",\"version\":\"0.1.0\"}}}}}}", id);
        } else if line.contains("\"method\":\"listTools\"") {
            println!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"tools\":[
                {{\"name\":\"batch\",\"description\":\"Execute a batch of MCP calls\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{{\"batch\":{{\"type\":\"string\",\"description\":\"JSON array of MCP calls\"}}}},\"required\":[\"batch\"]}}}},
                {{\"name\":\"packet_load\",\"description\":\"Load a context packet\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{{\"name\":{{\"type\":\"string\"}}}},\"required\":[\"name\"]}}}},
                {{\"name\":\"lease_touch\",\"description\":\"Touch a context lease\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{{\"id\":{{\"type\":\"string\"}}}},\"required\":[\"id\"]}}}},
                {{\"name\":\"skill_run\",\"description\":\"Run a predefined skill\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{{\"name\":{{\"type\":\"string\"}}}},\"required\":[\"name\"]}}}}
            ]}}}}", id);
        } else if line.contains("\"method\":\"callTool\"") {
            let tool_name = json_str_field(&line, "name").unwrap_or("");
            let params = line.find("\"params\"").and_then(|p| line[p..].find('{')).map(|p| {
                let start = line.find("\"params\"").unwrap() + p;
                extract_json_object(&line[start..])
            }).unwrap_or("");

            let result = match tool_name {
                "batch" => {
                    let batch_json = json_str_field(params, "batch").unwrap_or("[]");
                    match parse_batch_calls(batch_json) {
                        Ok(calls) => {
                            let mut results = Vec::new();
                            for (tool, arg) in calls {
                                let (status, payload) = execute_batch_call(&tool, arg.as_deref());
                                results.push(format!("{{\"tool\":\"{}\",\"status\":\"{}\",\"result\":\"{}\"}}", json_escape(&tool), status, json_escape(&payload)));
                            }
                            format!("[{}]", results.join(","))
                        }
                        Err(e) => format!("{{\"error\":\"{}\"}}", json_escape(&e))
                    }
                }
                "packet_load" => {
                    let name = json_str_field(params, "name").unwrap_or("");
                    match load_packet_as_batch(name) {
                        Ok(batch) => batch,
                        Err(e) => format!("{{\"error\":\"{}\"}}", json_escape(&e))
                    }
                }
                "lease_touch" => {
                    let lease_id = json_str_field(params, "id").unwrap_or("");
                    match touch_lease_logic(lease_id) {
                        Ok(json) => json,
                        Err(e) => format!("{{\"error\":\"{}\"}}", json_escape(&e))
                    }
                }
                "skill_run" => {
                    let name = json_str_field(params, "name").unwrap_or("");
                    match run_skill_logic(name) {
                        Ok(report) => report,
                        Err(e) => format!("{{\"error\":\"{}\"}}", json_escape(&e))
                    }
                }
                _ => format!("{{\"error\":\"unknown tool: {}\"}}", json_escape(tool_name))
            };

            println!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}]}}}}", id, json_escape(&result));
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn json_num_field<'a>(obj: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\"", key);
    let key_pos = obj.find(&needle)?;
    let after_key = &obj[key_pos + needle.len()..];
    let colon_pos = after_key.find(':')?;
    let val_start = after_key[colon_pos + 1..].find(|c: char| c.is_ascii_digit())?;
    let abs_start = key_pos + needle.len() + colon_pos + 1 + val_start;
    let val_end = obj[abs_start..].find(|c: char| !c.is_ascii_digit()).unwrap_or(obj.len() - abs_start);
    Some(&obj[abs_start..abs_start + val_end])
}

fn extract_json_object(input: &str) -> &str {
    let mut depth = 0;
    let mut end = 0;
    for (i, b) in input.as_bytes().iter().enumerate() {
        if *b == b'{' { depth += 1; }
        else if *b == b'}' {
            depth -= 1;
            if depth == 0 {
                end = i + 1;
                break;
            }
        }
    }
    if end > 0 { &input[..end] } else { "" }
}

fn expand_mcp_alias_with_symbols(input: &str) -> Result<mcp_router::McpCall, String> {
    let aliases = load_mcp_aliases()?;
    let mut expanded = expand_mcp_alias(input, &aliases)?;
    let symbol_aliases = load_symbol_aliases()?;
    expanded.argument_value = resolve_symbol_alias(&expanded.argument_value, &symbol_aliases);
    Ok(expanded)
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
            "/tmp/rtk_test_{}.txt",
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
