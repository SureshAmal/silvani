use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use anyhow::Result;
use gpui_kit::{App, SharedString};
use gpui_kit::component::{Theme, ThemeConfig, ThemeRegistry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSetting {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_providers() -> Vec<ProviderSetting> {
    vec![
        ProviderSetting { id: "hianime".to_string(), name: "HiAnime".to_string(), enabled: true },
        ProviderSetting { id: "gogoanime".to_string(), name: "GogoAnime".to_string(), enabled: true },
        ProviderSetting { id: "muse_india".to_string(), name: "Muse India (YT)".to_string(), enabled: true },
        ProviderSetting { id: "ani_one_asia".to_string(), name: "Ani-One Asia (YT)".to_string(), enabled: true },
        ProviderSetting { id: "animeworldindia".to_string(), name: "AnimeWorld India (HI)".to_string(), enabled: true },
        ProviderSetting { id: "desidubanime".to_string(), name: "DesiDub Anime (HI)".to_string(), enabled: true },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub mode: String,
    pub default_provider: String,
    pub default_audio: String,
    pub default_resolution: String,
    #[serde(default = "default_providers")]
    pub providers: Vec<ProviderSetting>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "Dark".to_string(),
            mode: "dark".to_string(),
            default_provider: "hianime".to_string(),
            default_audio: "sub".to_string(),
            default_resolution: "1080p".to_string(),
            providers: default_providers(),
        }
    }
}

impl AppSettings {
    pub fn enabled_providers(&self) -> Vec<(String, String)> {
        self.providers
            .iter()
            .filter(|p| p.enabled)
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect()
    }

    pub fn get_default_provider(&self) -> String {
        if self.providers.iter().any(|p| p.enabled && p.id == self.default_provider) {
            self.default_provider.clone()
        } else if let Some(first) = self.providers.iter().find(|p| p.enabled) {
            first.id.clone()
        } else {
            "hianime".to_string()
        }
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("silvani")
}

pub fn theme_dir() -> PathBuf {
    config_dir().join("theme")
}

pub fn setting_path() -> PathBuf {
    config_dir().join("setting.json")
}

pub fn ensure_config_files() -> Result<()> {
    let _cfg_dir = config_dir();
    let thm_dir = theme_dir();
    fs::create_dir_all(&thm_dir)?;

    let set_file = setting_path();
    if !set_file.exists() {
        let default_settings = AppSettings::default();
        let content = serde_json::to_string_pretty(&default_settings)?;
        fs::write(&set_file, content)?;
    }

    Ok(())
}

pub fn load_settings() -> AppSettings {
    let file = setting_path();
    if let Ok(content) = fs::read_to_string(&file)
        && let Ok(settings) = serde_json::from_str::<AppSettings>(&content)
    {
        return settings;
    }
    AppSettings::default()
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let file = setting_path();
    let content = serde_json::to_string_pretty(settings)?;
    fs::write(file, content)?;
    Ok(())
}

/// Pre-loads any theme JSON files from theme_dir into ThemeRegistry
pub fn preload_custom_themes(cx: &mut App) {
    let dir = theme_dir();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path.extension().and_then(|s| s.to_str()) == Some("json")
                && let Ok(content) = fs::read_to_string(&path)
            {
                let _ = ThemeRegistry::global_mut(cx).load_themes_from_str(&content);
            }
        }
    }
}

pub fn apply_theme(theme_name: &str, cx: &mut App) -> bool {
    let key = SharedString::from(theme_name.to_string());
    let target = ThemeRegistry::global(cx).themes().get(&key).cloned();

    if let Some(config) = target {
        Theme::update(cx, |theme| theme.apply_config(&config));
        Theme::change(config.mode, None, cx);

        // Update setting.json
        let mut settings = load_settings();
        settings.theme = theme_name.to_string();
        settings.mode = if config.mode.is_dark() {
            "dark".to_string()
        } else {
            "light".to_string()
        };
        let _ = save_settings(&settings);
        true
    } else {
        false
    }
}

pub fn available_themes(cx: &App) -> Vec<Rc<ThemeConfig>> {
    ThemeRegistry::global(cx).sorted_themes().into_iter().cloned().collect()
}

/// Watch themes directory and automatically re-apply current theme when files change (e.g. Matugen wall change)
pub fn watch_theme_changes(cx: &mut App) {
    let dir = theme_dir();
    cx.spawn(async move |cx| {
        use notify::Watcher as _;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher = match notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        }) {
            Ok(w) => w,
            Err(_) => return,
        };

        if watcher.watch(&dir, notify::RecursiveMode::NonRecursive).is_err() {
            return;
        }

        while rx.recv().await.is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(60)).await;
            cx.update(|cx| {
                preload_custom_themes(cx);
                let current_settings = load_settings();
                apply_theme(&current_settings.theme, cx);
            });
        }

    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_flow() {
        let _ = ensure_config_files();
        let s = load_settings();
        assert!(!s.theme.is_empty());
        assert!(!s.default_provider.is_empty());

        let mut updated = s.clone();
        updated.theme = "Dark".to_string();
        save_settings(&updated).expect("Save settings");

        let reloaded = load_settings();
        assert_eq!(reloaded.theme, "Dark");

        // Restore original
        let _ = save_settings(&s);

        // Test theme loading
        let dir = theme_dir();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let content = fs::read_to_string(&path).unwrap();
                    let res = serde_json::from_str::<gpui_kit::component::ThemeSet>(&content);
                    match res {
                        Ok(set) => println!("SUCCESS {}: {} themes", path.display(), set.themes.len()),
                        Err(err) => eprintln!("FAILED {}: {}", path.display(), err),
                    }
                }
            }
        }
    }
}
