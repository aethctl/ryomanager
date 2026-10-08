// systemd unit metadata comes from batched `systemctl show` calls; successful names are cached
// for the process lifetime, while missing units are retried slowly.
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::time::{Duration, Instant};

const NEW_UNIT_QUERY_INTERVAL: Duration = Duration::from_secs(5);
const FAILED_UNIT_RETRY_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct UnitDescriptions {
    known: HashMap<String, String>,
    attempted_at: HashMap<String, Instant>,
    last_new_query: Option<Instant>,
}

fn parse_descriptions(output: &[u8]) -> HashMap<String, String> {
    let mut descriptions = HashMap::new();
    let mut unit_id: Option<String> = None;
    let mut description: Option<String> = None;

    for line in String::from_utf8_lossy(output)
        .lines()
        .chain(std::iter::once(""))
    {
        if line.is_empty() {
            if let (Some(id), Some(value)) = (unit_id.take(), description.take()) {
                if !value.is_empty() && value != id {
                    descriptions.insert(id, value);
                }
            }
            continue;
        }

        if let Some(value) = line.strip_prefix("Id=") {
            unit_id = Some(value.to_string());
        }
        if let Some(value) = line.strip_prefix("Description=") {
            description = Some(value.to_string());
        }
    }

    descriptions
}

fn query(units: &[String], user_scope: bool) -> HashMap<String, String> {
    if units.is_empty() {
        return HashMap::new();
    }

    let mut command = Command::new("systemctl");
    if user_scope {
        command.arg("--user");
    }
    command.args(["show", "-p", "Id,Description"]);
    command.args(units);

    let Ok(output) = command.output() else {
        return HashMap::new();
    };

    parse_descriptions(&output.stdout)
}

impl UnitDescriptions {
    pub fn refresh<'a>(&mut self, units: impl Iterator<Item = &'a String>) {
        let now = Instant::now();
        let unresolved: HashSet<String> = units
            .filter(|unit| !self.known.contains_key(*unit))
            .cloned()
            .collect();
        let new_query_due = self
            .last_new_query
            .map(|last_query| last_query.elapsed() >= NEW_UNIT_QUERY_INTERVAL)
            .unwrap_or(true);
        let mut query_units = Vec::new();
        let mut includes_new_unit = false;

        for unit in unresolved {
            match self.attempted_at.get(&unit) {
                None if new_query_due => {
                    includes_new_unit = true;
                    query_units.push(unit);
                }
                Some(last_attempt) if last_attempt.elapsed() >= FAILED_UNIT_RETRY_INTERVAL => {
                    query_units.push(unit);
                }
                _ => {}
            }
        }

        if query_units.is_empty() {
            return;
        }

        for unit in &query_units {
            self.attempted_at.insert(unit.clone(), now);
        }
        if includes_new_unit {
            self.last_new_query = Some(now);
        }

        for (unit, description) in query(&query_units, true) {
            self.known.entry(unit).or_insert(description);
        }
        for (unit, description) in query(&query_units, false) {
            self.known.entry(unit).or_insert(description);
        }
    }

    pub fn get(&self, unit: &str) -> Option<&str> {
        self.known.get(unit).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_not_found_descriptions() {
        let descriptions = parse_descriptions(
            b"Id=ryoku-shell.service\nDescription=Ryoku Shell\n\n\
              Id=niri.service\nDescription=niri.service\n",
        );

        assert_eq!(
            descriptions.get("ryoku-shell.service").map(String::as_str),
            Some("Ryoku Shell")
        );
        assert!(!descriptions.contains_key("niri.service"));
    }
}
