use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tool_tokens: u64,
    pub memory_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SavingsEstimate {
    pub baseline_tokens: u64,
    pub saved_tokens: u64,
    pub estimated_cost_saved: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionRecord {
    pub timestamp_seconds: u64,
    pub repo: String,
    pub feature: String,
    pub kind: String,
    pub compact: String,
    pub expanded: String,
    pub status: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompressionSummary {
    pub records: u64,
    pub failed_records: u64,
    pub compact_chars: u64,
    pub expanded_chars: u64,
    pub saved_chars: u64,
    pub estimated_saved_tokens: u64,
}

impl TokenUsage {
    pub fn total(self) -> u64 {
        self.input_tokens + self.output_tokens + self.tool_tokens + self.memory_tokens
    }
}

pub fn estimate_savings(
    usage: TokenUsage,
    reduction_ratio: f64,
    blended_cost_per_token: f64,
) -> SavingsEstimate {
    let clamped_ratio = reduction_ratio.clamp(0.0, 1.0);
    let baseline_tokens = usage.total();
    let saved_tokens = (baseline_tokens as f64 * clamped_ratio).round() as u64;

    SavingsEstimate {
        baseline_tokens,
        saved_tokens,
        estimated_cost_saved: saved_tokens as f64 * blended_cost_per_token,
    }
}

impl CompressionRecord {
    pub fn new(
        kind: impl Into<String>,
        compact: impl Into<String>,
        expanded: impl Into<String>,
    ) -> Self {
        Self::with_metadata(0, "unknown", "default", kind, compact, expanded, "unknown")
    }

    pub fn with_metadata(
        timestamp_seconds: u64,
        repo: impl Into<String>,
        feature: impl Into<String>,
        kind: impl Into<String>,
        compact: impl Into<String>,
        expanded: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            timestamp_seconds,
            repo: repo.into(),
            feature: feature.into(),
            kind: kind.into(),
            compact: compact.into(),
            expanded: expanded.into(),
            status: status.into(),
        }
    }

    pub fn saved_chars(&self) -> u64 {
        self.expanded
            .chars()
            .count()
            .saturating_sub(self.compact.chars().count()) as u64
    }

    pub fn estimated_saved_tokens(&self) -> u64 {
        chars_to_tokens(self.saved_chars())
    }

    pub fn to_tsv_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            self.timestamp_seconds,
            escape_field(&self.repo),
            escape_field(&self.feature),
            escape_field(&self.kind),
            escape_field(&self.compact),
            escape_field(&self.expanded),
            escape_field(&self.status)
        )
    }

    pub fn to_compact_json(&self) -> String {
        format!(
            "{{\"timestamp_seconds\":{},\"repo\":\"{}\",\"feature\":\"{}\",\"kind\":\"{}\",\"compact\":\"{}\",\"expanded\":\"{}\",\"status\":\"{}\",\"saved_chars\":{},\"estimated_saved_tokens\":{}}}",
            self.timestamp_seconds,
            escape_json(&self.repo),
            escape_json(&self.feature),
            escape_json(&self.kind),
            escape_json(&self.compact),
            escape_json(&self.expanded),
            escape_json(&self.status),
            self.saved_chars(),
            self.estimated_saved_tokens()
        )
    }
}

impl CompressionSummary {
    pub fn add_record(&mut self, record: &CompressionRecord) {
        self.records += 1;
        if record.status.starts_with("exit:") && record.status != "exit:0" {
            self.failed_records += 1;
        }
        self.compact_chars += record.compact.chars().count() as u64;
        self.expanded_chars += record.expanded.chars().count() as u64;
        self.saved_chars += record.saved_chars();
        self.estimated_saved_tokens += record.estimated_saved_tokens();
    }

    pub fn estimated_cost_saved(self, blended_cost_per_token: f64) -> f64 {
        self.estimated_saved_tokens as f64 * blended_cost_per_token
    }

    pub fn to_compact_json(self, blended_cost_per_token: f64) -> String {
        format!(
            "{{\"records\":{},\"failed_records\":{},\"compact_chars\":{},\"expanded_chars\":{},\"saved_chars\":{},\"estimated_saved_tokens\":{},\"estimated_cost_saved\":{:.6}}}",
            self.records,
            self.failed_records,
            self.compact_chars,
            self.expanded_chars,
            self.saved_chars,
            self.estimated_saved_tokens,
            self.estimated_cost_saved(blended_cost_per_token)
        )
    }
}

pub fn summarize_compression(records: &[CompressionRecord]) -> CompressionSummary {
    let mut summary = CompressionSummary::default();
    for record in records {
        summary.add_record(record);
    }
    summary
}

pub fn summarize_compression_by_kind(
    records: &[CompressionRecord],
) -> Vec<(String, CompressionSummary)> {
    summarize_compression_by(records, |record| record.kind.clone())
}

pub fn summarize_compression_by_repo(
    records: &[CompressionRecord],
) -> Vec<(String, CompressionSummary)> {
    summarize_compression_by(records, |record| record.repo.clone())
}

pub fn summarize_compression_by_feature(
    records: &[CompressionRecord],
) -> Vec<(String, CompressionSummary)> {
    summarize_compression_by(records, |record| record.feature.clone())
}

pub fn summarize_compression_by_context(
    records: &[CompressionRecord],
) -> Vec<(String, CompressionSummary)> {
    summarize_compression_by(records, |record| {
        format!("{}:{}", record.repo, record.feature)
    })
}

fn summarize_compression_by(
    records: &[CompressionRecord],
    key: impl Fn(&CompressionRecord) -> String,
) -> Vec<(String, CompressionSummary)> {
    let mut summaries = BTreeMap::<String, CompressionSummary>::new();

    for record in records {
        summaries.entry(key(record)).or_default().add_record(record);
    }

    summaries.into_iter().collect()
}

pub fn parse_compression_history(input: &str) -> Result<Vec<CompressionRecord>, String> {
    input
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.trim().is_empty() {
                None
            } else {
                Some(parse_history_line(index + 1, line))
            }
        })
        .collect()
}

fn parse_history_line(line_number: usize, line: &str) -> Result<CompressionRecord, String> {
    let fields = line.split('\t').collect::<Vec<_>>();

    if fields.len() == 3 {
        return Ok(CompressionRecord::new(
            unescape_field(fields[0])?,
            unescape_field(fields[1])?,
            unescape_field(fields[2])?,
        ));
    }

    if fields.len() == 5 {
        let timestamp_seconds = fields[0]
            .parse::<u64>()
            .map_err(|error| format!("line {line_number}: invalid timestamp: {error}"))?;

        return Ok(CompressionRecord::with_metadata(
            timestamp_seconds,
            "unknown",
            "default",
            unescape_field(fields[1])?,
            unescape_field(fields[2])?,
            unescape_field(fields[3])?,
            unescape_field(fields[4])?,
        ));
    }

    if fields.len() != 7 {
        return Err(format!(
            "line {line_number}: expected 3, 5, or 7 tab-separated fields"
        ));
    }

    let timestamp_seconds = fields[0]
        .parse::<u64>()
        .map_err(|error| format!("line {line_number}: invalid timestamp: {error}"))?;

    Ok(CompressionRecord::with_metadata(
        timestamp_seconds,
        unescape_field(fields[1])?,
        unescape_field(fields[2])?,
        unescape_field(fields[3])?,
        unescape_field(fields[4])?,
        unescape_field(fields[5])?,
        unescape_field(fields[6])?,
    ))
}

fn chars_to_tokens(chars: u64) -> u64 {
    chars.div_ceil(4)
}

fn escape_field(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn unescape_field(input: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        match chars.next() {
            Some('\\') => output.push('\\'),
            Some('t') => output.push('\t'),
            Some('n') => output.push('\n'),
            Some(other) => return Err(format!("unsupported escape sequence: \\{other}")),
            None => return Err("trailing escape in field".to_string()),
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_savings() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            tool_tokens: 40,
            memory_tokens: 10,
        };
        let estimate = estimate_savings(usage, 0.5, 0.01);
        assert_eq!(estimate.baseline_tokens, 200);
        assert_eq!(estimate.saved_tokens, 100);
        assert_eq!(estimate.estimated_cost_saved, 1.0);
    }

    #[test]
    fn summarizes_compression_records() {
        let records = vec![
            CompressionRecord::new("cmd", "gs", "git status --short"),
            CompressionRecord::new(
                "mcp",
                "rf @authrepo",
                "{\"tool\":\"read_file\",\"path\":\"x\"}",
            ),
        ];

        let summary = summarize_compression(&records);

        assert_eq!(summary.records, 2);
        assert_eq!(summary.failed_records, 0);
        assert!(summary.saved_chars > 0);
        assert!(summary.estimated_saved_tokens > 0);
    }

    #[test]
    fn summarizes_by_kind_and_counts_failures() {
        let records = vec![
            CompressionRecord::with_metadata(
                1,
                "agent-kosh",
                "default",
                "cmd",
                "gs",
                "git status --short",
                "exit:0",
            ),
            CompressionRecord::with_metadata(
                2,
                "agent-kosh",
                "default",
                "cmd",
                "bad",
                "missing command",
                "exit:127",
            ),
            CompressionRecord::with_metadata(
                3,
                "agent-kosh",
                "auth",
                "mcp",
                "rf @a",
                "{\"tool\":\"read_file\"}",
                "ok",
            ),
        ];

        let summaries = summarize_compression_by_kind(&records);

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].0, "cmd");
        assert_eq!(summaries[0].1.records, 2);
        assert_eq!(summaries[0].1.failed_records, 1);
        assert_eq!(summaries[1].0, "mcp");

        let by_context = summarize_compression_by_context(&records);
        assert_eq!(by_context.len(), 2);
        assert_eq!(by_context[0].0, "agent-kosh:auth");
        assert_eq!(by_context[1].0, "agent-kosh:default");
    }

    #[test]
    fn round_trips_compression_history_line() {
        let record = CompressionRecord::with_metadata(
            123,
            "agent-kosh",
            "auth",
            "cmd",
            "a\tb",
            "line one\nline two",
            "exit:0",
        );
        let parsed = parse_compression_history(&record.to_tsv_line()).unwrap();
        assert_eq!(parsed, vec![record]);
    }

    #[test]
    fn emits_compact_json() {
        let record = CompressionRecord::with_metadata(
            123,
            "agent-kosh",
            "auth",
            "cmd",
            "a\tb",
            "line one\nline two",
            "exit:0",
        );

        assert_eq!(
            record.to_compact_json(),
            "{\"timestamp_seconds\":123,\"repo\":\"agent-kosh\",\"feature\":\"auth\",\"kind\":\"cmd\",\"compact\":\"a\\tb\",\"expanded\":\"line one\\nline two\",\"status\":\"exit:0\",\"saved_chars\":14,\"estimated_saved_tokens\":4}"
        );
    }

    #[test]
    fn parses_previous_five_field_history() {
        let parsed = parse_compression_history("123\tcmd\tgs\tgit status --short\texit:0\n")
            .expect("valid history");

        assert_eq!(
            parsed,
            vec![CompressionRecord::with_metadata(
                123,
                "unknown",
                "default",
                "cmd",
                "gs",
                "git status --short",
                "exit:0"
            )]
        );
    }

    #[test]
    fn parses_legacy_three_field_history() {
        let parsed =
            parse_compression_history("cmd\tgs\tgit status --short\n").expect("valid history");

        assert_eq!(
            parsed,
            vec![CompressionRecord::new("cmd", "gs", "git status --short")]
        );
    }
}
