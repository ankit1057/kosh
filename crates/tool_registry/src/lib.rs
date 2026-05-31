#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandAlias {
    pub alias: Vec<String>,
    pub expansion: Vec<String>,
}

impl CommandAlias {
    pub fn new(alias: &[&str], expansion: &[&str]) -> Self {
        Self {
            alias: alias.iter().map(|part| part.to_string()).collect(),
            expansion: expansion.iter().map(|part| part.to_string()).collect(),
        }
    }

    pub fn from_parts(alias: Vec<String>, expansion: Vec<String>) -> Result<Self, String> {
        if alias.is_empty() {
            return Err("command alias cannot be empty".to_string());
        }
        if expansion.is_empty() {
            return Err("command alias expansion cannot be empty".to_string());
        }

        Ok(Self { alias, expansion })
    }
}

pub fn default_aliases() -> Vec<CommandAlias> {
    vec![
        CommandAlias::new(&["gs"], &["git", "status", "--short"]),
        CommandAlias::new(&["gd"], &["git", "diff"]),
        CommandAlias::new(&["gl"], &["git", "log", "--oneline", "-20"]),
        CommandAlias::new(&["gb"], &["git", "branch", "--show-current"]),
        CommandAlias::new(&["fpg"], &["flutter", "pub", "get"]),
        CommandAlias::new(&["ft"], &["flutter", "test"]),
        CommandAlias::new(&["dart", "files"], &["find", ".", "-name", "*.dart"]),
        CommandAlias::new(&["rust", "files"], &["find", ".", "-name", "*.rs"]),
    ]
}

pub fn expand_command(input: &[String], aliases: &[CommandAlias]) -> Vec<String> {
    let Some(alias) = aliases
        .iter()
        .filter(|candidate| starts_with(input, &candidate.alias))
        .max_by_key(|candidate| candidate.alias.len())
    else {
        return input.to_vec();
    };

    let mut expanded = alias.expansion.clone();
    expanded.extend_from_slice(&input[alias.alias.len()..]);
    expanded
}

pub fn parse_aliases(input: &str) -> Result<Vec<CommandAlias>, String> {
    input
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                None
            } else {
                Some(parse_alias_line(index + 1, trimmed))
            }
        })
        .collect()
}

fn parse_alias_line(line_number: usize, line: &str) -> Result<CommandAlias, String> {
    let Some((alias, expansion)) = line.split_once("=>") else {
        return Err(format!(
            "line {line_number}: expected '<alias> => <expansion>'"
        ));
    };

    CommandAlias::from_parts(split_words(alias), split_words(expansion))
        .map_err(|error| format!("line {line_number}: {error}"))
}

fn split_words(input: &str) -> Vec<String> {
    input.split_whitespace().map(ToString::to_string).collect()
}

fn starts_with(input: &[String], prefix: &[String]) -> bool {
    input.len() >= prefix.len()
        && input
            .iter()
            .zip(prefix.iter())
            .all(|(left, right)| left == right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_single_word_alias() {
        let input = vec!["gs".to_string()];
        assert_eq!(
            expand_command(&input, &default_aliases()),
            vec!["git", "status", "--short"]
        );
    }

    #[test]
    fn expands_multi_word_alias_and_preserves_extra_args() {
        let input = vec!["dart".to_string(), "files".to_string(), "lib".to_string()];
        assert_eq!(
            expand_command(&input, &default_aliases()),
            vec!["find", ".", "-name", "*.dart", "lib"]
        );
    }

    #[test]
    fn leaves_unknown_command_unchanged() {
        let input = vec!["cargo".to_string(), "test".to_string()];
        assert_eq!(expand_command(&input, &default_aliases()), input);
    }

    #[test]
    fn parses_alias_config() {
        let aliases = parse_aliases(
            r#"
            # git
            gst => git status
            dart files => find . -name *.dart
            "#,
        )
        .unwrap();

        assert_eq!(
            aliases,
            vec![
                CommandAlias::new(&["gst"], &["git", "status"]),
                CommandAlias::new(&["dart", "files"], &["find", ".", "-name", "*.dart"])
            ]
        );
    }
}
