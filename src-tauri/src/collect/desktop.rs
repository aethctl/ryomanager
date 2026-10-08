// Desktop identities come from XDG application files and icon themes; entries refresh every
// minute, while resolved icon data is cached for the collector lifetime.
use base64::Engine;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct DesktopEntry {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub startup_class: Option<String>,
    pub exec_base: Option<String>,
}

#[derive(Default)]
pub struct DesktopIndex {
    entries: Vec<DesktopEntry>,
    refreshed: Option<Instant>,
    icons: HashMap<String, Option<String>>,
}

fn data_dirs() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/share/applications"));
    }
    for directory in std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".into())
        .split(':')
    {
        directories.push(PathBuf::from(directory).join("applications"));
    }

    directories
}

fn parse_desktop(path: &Path) -> Option<DesktopEntry> {
    let text = fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut values = HashMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry {
            if let Some((key, value)) = line.split_once('=') {
                values
                    .entry(key.to_string())
                    .or_insert_with(|| value.to_string());
            }
        }
    }
    if values.get("Type").map(String::as_str) != Some("Application")
        || values.get("NoDisplay").map(String::as_str) == Some("true")
    {
        return None;
    }

    let id = path
        .file_name()?
        .to_string_lossy()
        .trim_end_matches(".desktop")
        .to_string();
    let exec_base = values
        .get("Exec")
        .and_then(|command: &String| {
            command
                .split_whitespace()
                .find(|part| !part.contains('=') && !part.starts_with('%'))
        })
        .and_then(|executable| Path::new(executable).file_name())
        .map(|name| name.to_string_lossy().into_owned());

    Some(DesktopEntry {
        id,
        name: values
            .get("Name")
            .cloned()
            .unwrap_or_else(|| "Application".into()),
        icon: values.get("Icon").cloned(),
        startup_class: values.get("StartupWMClass").cloned(),
        exec_base,
    })
}

fn scan() -> Vec<DesktopEntry> {
    let mut entries_by_id = HashMap::new();
    for directory in data_dirs() {
        for entry in WalkDir::new(directory)
            .max_depth(3)
            .into_iter()
            .flatten()
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|extension| extension.to_str())
                    == Some("desktop")
            })
        {
            if let Some(desktop_entry) = parse_desktop(entry.path()) {
                entries_by_id
                    .entry(desktop_entry.id.to_ascii_lowercase())
                    .or_insert(desktop_entry);
            }
        }
    }

    entries_by_id.into_values().collect()
}

fn scope_tokens(scope: &str) -> Vec<String> {
    scope
        .trim_end_matches(".scope")
        .strip_prefix("app-")
        .unwrap_or(scope)
        .replace("\\x2d", "-")
        .split('-')
        .filter(|token| {
            !matches!(*token, "ryoku" | "app" | "uwsm" | "sh" | "bash" | "niri")
                && !token.chars().all(|character| character.is_ascii_digit())
        })
        .map(str::to_string)
        .collect()
}

impl DesktopIndex {
    pub fn refresh(&mut self) {
        if self
            .refreshed
            .map(|refreshed| refreshed.elapsed() >= Duration::from_secs(60))
            .unwrap_or(true)
        {
            self.entries = scan();
            self.refreshed = Some(Instant::now())
        }
    }
    pub fn resolve(
        &self,
        app_ids: &[String],
        scope: Option<&str>,
        exe: Option<&str>,
    ) -> Option<&DesktopEntry> {
        for app in app_ids {
            let candidate = app.trim_end_matches(".desktop");
            if let Some(entry) = self.entries.iter().find(|entry| {
                entry.id.eq_ignore_ascii_case(candidate)
                    || entry
                        .startup_class
                        .as_deref()
                        .map(|class| class.eq_ignore_ascii_case(candidate))
                        .unwrap_or(false)
            }) {
                return Some(entry);
            }
        }
        for token in scope.map(scope_tokens).unwrap_or_default() {
            if let Some(entry) = self.entries.iter().find(|entry| {
                entry.id.eq_ignore_ascii_case(&token)
                    || entry
                        .startup_class
                        .as_deref()
                        .map(|class| class.eq_ignore_ascii_case(&token))
                        .unwrap_or(false)
            }) {
                return Some(entry);
            }
        }
        if let Some(base) = exe
            .and_then(|path| Path::new(path).file_name())
            .and_then(|name| name.to_str())
        {
            if let Some(entry) = self.entries.iter().find(|entry| {
                entry
                    .exec_base
                    .as_deref()
                    .map(|exec| exec.eq_ignore_ascii_case(base))
                    .unwrap_or(false)
            }) {
                return Some(entry);
            }
        }
        None
    }
    pub fn entry(&self, id: &str) -> Option<&DesktopEntry> {
        self.entries
            .iter()
            .find(|entry| entry.id.eq_ignore_ascii_case(id))
    }

    pub fn icon_for_key(&mut self, key: &str) -> Option<String> {
        if let Some(cached) = self.icons.get(key) {
            return cached.clone();
        }
        let icon = self
            .entry(key)
            .and_then(|entry| entry.icon.clone())
            .unwrap_or_else(|| key.into());
        let found = icon_data(&icon);
        self.icons.insert(key.into(), found.clone());
        found
    }

    #[cfg(test)]
    pub(crate) fn from_entries(entries: Vec<DesktopEntry>) -> Self {
        Self {
            entries,
            refreshed: Some(Instant::now()),
            icons: HashMap::new(),
        }
    }
}

fn gtk_theme() -> String {
    let settings_path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/gtk-3.0/settings.ini"));
    if let Some(Ok(text)) = settings_path.map(fs::read_to_string) {
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix("gtk-icon-theme-name=") {
                return value.trim_matches('"').into();
            }
        }
    }

    "Adwaita".into()
}

fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".icons"));
        roots.push(home.join(".local/share/icons"));
    }
    roots.push("/usr/share/icons".into());

    roots
}

fn inherits(theme: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![theme.to_string()];
    let mut seen = HashSet::new();
    while let Some(current) = pending.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }
        out.push(current.clone());
        for root in roots() {
            if let Ok(text) = fs::read_to_string(root.join(&current).join("index.theme")) {
                for line in text.lines() {
                    if let Some(value) = line.strip_prefix("Inherits=") {
                        pending.extend(
                            value
                                .split(',')
                                .map(|name| name.trim().to_string())
                                .filter(|name| !name.is_empty()),
                        );
                    }
                }
            }
        }
    }
    if !seen.contains("hicolor") {
        out.push("hicolor".into())
    }
    out
}

fn find_icon(name: &str) -> Option<PathBuf> {
    let requested_path = Path::new(name);
    if requested_path.is_absolute() && requested_path.is_file() {
        return Some(requested_path.into());
    }
    for theme in inherits(&gtk_theme()) {
        for root in roots() {
            let directory = root.join(&theme);
            let mut choices: Vec<_> = WalkDir::new(directory)
                .max_depth(5)
                .into_iter()
                .flatten()
                .filter(|entry| entry.file_type().is_file())
                .filter(|entry| {
                    entry
                        .path()
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(|stem| stem.eq_ignore_ascii_case(name))
                        .unwrap_or(false)
                })
                .map(|entry| entry.into_path())
                .collect();
            choices.sort_by_key(|path| {
                if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
                    0
                } else {
                    1
                }
            });
            if let Some(path) = choices.into_iter().next() {
                return Some(path);
            }
        }
    }
    for extension in ["svg", "png", "xpm"] {
        let path = PathBuf::from(format!("/usr/share/pixmaps/{name}.{extension}"));
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

fn icon_data(name: &str) -> Option<String> {
    let path = find_icon(name)?;
    let bytes = fs::read(&path).ok()?;
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "xpm" => "image/x-xpixmap",
        _ => return None,
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
