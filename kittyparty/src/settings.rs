//! Tiny key=value settings file in %APPDATA%\KittyParty, plus where her own
//! pictures and custom kitty live.

use std::path::{Path, PathBuf};

pub struct Settings {
    pub rain_level: usize,
    pub click_bursts: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { rain_level: 1, click_bursts: true }
    }
}

/// %APPDATA%\KittyParty, created on first run.
pub fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("KittyParty");
    std::fs::create_dir_all(dir.join("My Cats")).ok()?;
    let note = dir.join("README.txt");
    if !note.exists() {
        let _ = std::fs::write(
            &note,
            "KittyParty\r\n\
             \r\n\
             * Drop your own pictures (jpg, png, gif, webp) into the \"My Cats\" folder.\r\n\
             \x20 They join the cat confetti next time KittyParty starts.\r\n\
             * Want a different kitty? Save a picture here named kitty.png\r\n\
             \x20 (a transparent PNG looks best) and restart KittyParty.\r\n",
        );
    }
    Some(dir)
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        let mut s = Self::default();
        let Ok(text) = std::fs::read_to_string(dir.join("settings.txt")) else {
            return s;
        };
        for line in text.lines() {
            match line.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("rain", v)) => s.rain_level = v.parse::<usize>().unwrap_or(1).min(2),
                Some(("click_bursts", v)) => s.click_bursts = v != "0",
                _ => {}
            }
        }
        s
    }

    pub fn save(&self, dir: &Path) {
        let text = format!("rain={}\nclick_bursts={}\n", self.rain_level, self.click_bursts as u8);
        let _ = std::fs::write(dir.join("settings.txt"), text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let dir = std::env::temp_dir().join(format!("kittyparty-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        Settings { rain_level: 2, click_bursts: false }.save(&dir);
        let s = Settings::load(&dir);
        assert_eq!(s.rain_level, 2);
        assert!(!s.click_bursts);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
