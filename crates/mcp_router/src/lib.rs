#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpAlias {
    pub alias: String,
    pub tool: String,
    pub argument_name: String,
}

impl McpAlias {
    pub fn new(
        alias: impl Into<String>,
        tool: impl Into<String>,
        argument_name: impl Into<String>,
    ) -> Self {
        Self {
            alias: alias.into(),
            tool: tool.into(),
            argument_name: argument_name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpCall {
    pub tool: String,
    pub argument_name: String,
    pub argument_value: String,
}

impl McpCall {
    pub fn to_compact_json(&self) -> String {
        format!(
            "{{\"tool\":\"{}\",\"{}\":\"{}\"}}",
            escape_json(&self.tool),
            escape_json(&self.argument_name),
            escape_json(&self.argument_value)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolAlias {
    pub symbol: String,
    pub value: String,
}

impl SymbolAlias {
    pub fn new(symbol: impl Into<String>, value: impl Into<String>) -> Result<Self, String> {
        let symbol = symbol.into();
        if !symbol.starts_with('@') {
            return Err("symbol aliases must start with '@'".to_string());
        }

        let value = value.into();
        if value.trim().is_empty() {
            return Err("symbol alias value cannot be empty".to_string());
        }

        Ok(Self { symbol, value })
    }
}

pub fn default_mcp_aliases() -> Vec<McpAlias> {
    vec![
        McpAlias::new("rf", "read_file", "path"),
        McpAlias::new("sf", "search_files", "query"),
        McpAlias::new("ls", "list_directory", "path"),
    ]
}

pub fn expand_mcp_alias(input: &str, aliases: &[McpAlias]) -> Result<McpCall, String> {
    let mut parts = input.split_whitespace();
    let alias_name = parts
        .next()
        .ok_or_else(|| "missing MCP alias".to_string())?;
    let argument_value = parts.collect::<Vec<_>>().join(" ");

    if argument_value.is_empty() {
        return Err("missing MCP alias argument".to_string());
    }

    let alias = aliases
        .iter()
        .find(|candidate| candidate.alias == alias_name)
        .ok_or_else(|| format!("unknown MCP alias: {alias_name}"))?;

    Ok(McpCall {
        tool: alias.tool.clone(),
        argument_name: alias.argument_name.clone(),
        argument_value,
    })
}

pub fn resolve_symbol_alias(argument_value: &str, aliases: &[SymbolAlias]) -> String {
    aliases
        .iter()
        .find(|candidate| candidate.symbol == argument_value)
        .map(|candidate| candidate.value.clone())
        .unwrap_or_else(|| argument_value.to_string())
}

pub fn parse_mcp_aliases(input: &str) -> Result<Vec<McpAlias>, String> {
    input
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                None
            } else {
                Some(parse_mcp_alias_line(index + 1, trimmed))
            }
        })
        .collect()
}

fn parse_mcp_alias_line(line_number: usize, line: &str) -> Result<McpAlias, String> {
    let Some((alias, expansion)) = line.split_once("=>") else {
        return Err(format!(
            "line {line_number}: expected '<alias> => <tool> <argument_name>'"
        ));
    };

    let alias = alias.trim();
    let mut expansion = expansion.split_whitespace();
    let tool = expansion
        .next()
        .ok_or_else(|| format!("line {line_number}: missing MCP tool"))?;
    let argument_name = expansion
        .next()
        .ok_or_else(|| format!("line {line_number}: missing MCP argument name"))?;

    if expansion.next().is_some() {
        return Err(format!(
            "line {line_number}: MCP aliases support exactly one argument name"
        ));
    }

    Ok(McpAlias::new(alias, tool, argument_name))
}

pub fn parse_symbol_aliases(input: &str) -> Result<Vec<SymbolAlias>, String> {
    input
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                None
            } else {
                Some(parse_symbol_alias_line(index + 1, trimmed))
            }
        })
        .collect()
}

fn parse_symbol_alias_line(line_number: usize, line: &str) -> Result<SymbolAlias, String> {
    let Some((symbol, value)) = line.split_once("=>") else {
        return Err(format!(
            "line {line_number}: expected '<symbol> => <value>'"
        ));
    };

    SymbolAlias::new(symbol.trim(), value.trim())
        .map_err(|error| format!("line {line_number}: {error}"))
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_read_file_alias() {
        let call = expand_mcp_alias("rf @authrepo", &default_mcp_aliases()).unwrap();
        assert_eq!(
            call,
            McpCall {
                tool: "read_file".to_string(),
                argument_name: "path".to_string(),
                argument_value: "@authrepo".to_string()
            }
        );
        assert_eq!(
            call.to_compact_json(),
            "{\"tool\":\"read_file\",\"path\":\"@authrepo\"}"
        );
    }

    #[test]
    fn rejects_unknown_alias() {
        let error = expand_mcp_alias("xx @authrepo", &default_mcp_aliases()).unwrap_err();
        assert!(error.contains("unknown MCP alias"));
    }

    #[test]
    fn parses_mcp_alias_config() {
        let aliases = parse_mcp_aliases(
            r#"
            # read file
            rf => read_file path
            sf => search_files query
            "#,
        )
        .unwrap();

        assert_eq!(
            aliases,
            vec![
                McpAlias::new("rf", "read_file", "path"),
                McpAlias::new("sf", "search_files", "query")
            ]
        );
    }

    #[test]
    fn parses_and_resolves_symbol_aliases() {
        let aliases = parse_symbol_aliases(
            r#"
            @authrepo => lib/features/auth/data/repositories/auth_repository_impl.dart
            "#,
        )
        .unwrap();

        assert_eq!(
            resolve_symbol_alias("@authrepo", &aliases),
            "lib/features/auth/data/repositories/auth_repository_impl.dart"
        );
        assert_eq!(resolve_symbol_alias("@missing", &aliases), "@missing");
    }
}
