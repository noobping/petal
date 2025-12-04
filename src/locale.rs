use dirs_next as dirs;
use gettextrs::{
    bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory,
};
use std::{env, fs, path::{Path, PathBuf}};

const APP_ID: &str = env!("APP_ID"); // or your crate's APP_ID constant

fn find_locale_dir() -> PathBuf {
    // Developer directory (cargo run)
    let dev_dir = Path::new("data").join("locale");
    if dev_dir.is_dir() {
        return dev_dir;
    }

    // <exe>/data/locale or <exe>/locale (AppImage, portable build, etc.)
    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join("data").join("locale");
            if candidate.is_dir() {
                return candidate;
            }

            let candidate = exe_dir.join("locale");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }

    // User-level data dir
    // Linux → ~/.local/share/<APP_ID>/locale
    // macOS → ~/Library/Application Support/<APP_ID>/locale
    // Windows → %APPDATA%\<APP_ID>\locale
    if let Some(base) = dirs::data_local_dir() {
        let candidate = base.join(APP_ID).join("locale");
        if candidate.is_dir() {
            return candidate;
        }
    }

    // System locale directory (/usr/share/locale)
    // `dirs-next` does not expose `/usr/share`, so we check it manually.
    let sys_dir = Path::new("/usr/share/locale");
    if sys_dir.is_dir() {
        return sys_dir.to_path_buf();
    }

    // Absolute fallback, ensure dev folder exists
    let _ = fs::create_dir_all(&dev_dir);
    dev_dir
}

pub fn init_i18n() {
    setlocale(LocaleCategory::LcAll, "");

    let dir = find_locale_dir();
    let dir_str = dir
        .to_str()
        .expect("Locale path must be UTF-8 for gettext");

    bindtextdomain(APP_ID, dir_str).expect("bindtextdomain failed");
    bind_textdomain_codeset(APP_ID, "UTF-8").expect("bind codeset failed");
    textdomain(APP_ID).expect("textdomain failed");
}

#[macro_export]
macro_rules! t {
    ($msg:literal) => {
        &gettextrs::gettext($msg)
    };
}

#[macro_export]
macro_rules! v {
    ($msg:literal) => {{
        let value = std::env::var($msg).unwrap_or_else(|_| panic!(concat!($msg, " not found")));
        gettextrs::gettext(&value)
    }};
}
