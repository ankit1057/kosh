use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillAction {
    pub kind: String, // "cmd" or "mcp"
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRecord {
    pub name: String,
    pub description: String,
    pub actions: Vec<SkillAction>,
}

impl SkillAction {
    pub fn new(kind: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            value: value.into(),
        }
    }

    pub fn to_string(&self) -> String {
        format!("{}:{}", self.kind, self.value)
    }

    pub fn from_string(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(format!("invalid skill action format: {}", s));
        }
        Ok(Self::new(parts[0], parts[1]))
    }
}

impl SkillRecord {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        actions: Vec<SkillAction>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            actions,
        }
    }

    pub fn to_compact_json(&self) -> String {
        let actions_json = self
            .actions
            .iter()
            .map(|a| {
                format!(
                    "{{\"kind\":\"{}\",\"value\":\"{}\"}}",
                    escape_json(&a.kind),
                    escape_json(&a.value)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"name\":\"{}\",\"description\":\"{}\",\"actions\":[{}]}}",
            escape_json(&self.name),
            escape_json(&self.description),
            actions_json
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillStore {
    records: Vec<SkillRecord>,
}

impl SkillStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = fs::read_to_string(path)?;
        Self::from_tsv(&contents).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_tsv())
    }

    pub fn upsert(&mut self, record: SkillRecord) {
        if let Some(existing) = self.records.iter_mut().find(|r| r.name == record.name) {
            *existing = record;
        } else {
            self.records.push(record);
        }
    }

    pub fn get(&self, name: &str) -> Option<&SkillRecord> {
        self.records.iter().find(|r| r.name == name)
    }

    pub fn records(&self) -> &[SkillRecord] {
        &self.records
    }

    pub fn from_tsv(input: &str) -> Result<Self, String> {
        let mut store = Self::new();

        for (index, line) in input.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() != 3 {
                return Err(format!(
                    "line {}: expected 3 tab-separated fields",
                    index + 1
                ));
            }

            let name = unescape_field(fields[0])?;
            let description = unescape_field(fields[1])?;
            let actions_str = fields[2];
            let mut actions = Vec::new();
            if !actions_str.is_empty() {
                for action_raw in actions_str.split('|') {
                    actions.push(SkillAction::from_string(&unescape_field(action_raw)?)?);
                }
            }

            store.upsert(SkillRecord::new(name, description, actions));
        }

        Ok(store)
    }

    pub fn to_tsv(&self) -> String {
        let mut output = String::new();

        for record in &self.records {
            output.push_str(&escape_field(&record.name));
            output.push('\t');
            output.push_str(&escape_field(&record.description));
            output.push('\t');
            let actions_str = record
                .actions
                .iter()
                .map(|a| escape_field(&a.to_string()))
                .collect::<Vec<_>>()
                .join("|");
            output.push_str(&actions_str);
            output.push('\n');
        }

        output
    }
}

fn escape_field(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
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
    fn round_trips_tsv() {
        let mut store = SkillStore::new();
        store.upsert(SkillRecord::new(
            "audit",
            "Audit the codebase",
            vec![
                SkillAction::new("cmd", "cargo check"),
                SkillAction::new("mcp", "rf @main"),
            ],
        ));

        let tsv = store.to_tsv();
        let restored = SkillStore::from_tsv(&tsv).expect("round-trip failed");
        assert_eq!(store, restored);
    }
}
