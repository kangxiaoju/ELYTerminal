//! The interface's words, English and Simplified Chinese, via `I18n`.

use std::sync::LazyLock;

use ely_gpui_component::i18n::I18n;
use gpui::{App, SharedString};

use crate::{
    menus,
    settings::{Language, SettingsStore},
};

mod en;
mod zh;

/// Every catalog, `language` shown; each must hold English's keys.
pub fn catalogs(language: Language) -> I18n {
    I18n::new(language.tag())
        .catalog(Language::English.tag(), en::MESSAGES)
        .catalog(Language::SimplifiedChinese.tag(), zh::MESSAGES)
}

/// Terminal catalogs stay separate from the host's application messages.
/// They are built once; each lookup follows the host's current locale.
static WORDS: LazyLock<[I18n; 2]> = LazyLock::new(|| {
    [catalogs(Language::English), catalogs(Language::SimplifiedChinese)]
});

fn for_locale(tag: &str) -> &'static I18n {
    match tag {
        "en-US" => &WORDS[0],
        "zh-CN" => &WORDS[1],
        _ => panic!("no terminal catalog for {tag}"),
    }
}

/// Terminal messages in the host's current locale, without replacing its catalog.
pub fn words(cx: &App) -> &'static I18n {
    for_locale(cx.global::<I18n>().locale().tag)
}

/// A terminal message, filled by Ely's normal message formatter.
pub fn text(cx: &App, key: &str, args: &[(&str, &str)]) -> SharedString {
    words(cx).text(key, args)
}

/// Speaks the settings' language everywhere, and follows its changes.
pub fn init(cx: &mut App) {
    let mut spoken = SettingsStore::get(cx).language;
    log::info!("i18n: the interface speaks {}", spoken.tag());
    cx.set_global(catalogs(spoken));
    cx.observe_global::<SettingsStore>(move |cx| {
        let language = SettingsStore::get(cx).language;
        if language != spoken {
            spoken = language;
            I18n::set_locale(language.tag(), cx);
            cx.set_menus(menus::menus(cx));
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use super::*;

    /// The `{name}`s a message fills.
    fn names(message: &str) -> BTreeSet<&str> {
        message
            .split('{')
            .skip(1)
            .map(|rest| rest.split_once('}').expect("a { ends").0)
            .collect()
    }

    /// The keys the source at `path` names, the catalogs aside.
    fn named(path: &Path, keys: &mut BTreeSet<String>) {
        if path.ends_with("i18n") || path.ends_with("i18n.rs") {
            return;
        }
        if path.is_dir() {
            for entry in fs::read_dir(path).expect("a source folder reads") {
                named(&entry.expect("an entry reads").path(), keys);
            }
            return;
        }
        let source = fs::read_to_string(path).expect("a source file reads");
        for rest in source.split("\"et.").skip(1) {
            let (key, _) = rest.split_once('"').expect("a key ends");
            keys.insert(format!("et.{key}"));
        }
    }

    #[test]
    fn each_catalog_holds_english_s_keys_and_fills_the_same_names() {
        for language in [Language::English, Language::SimplifiedChinese] {
            assert_eq!(catalogs(language).locale().tag, language.tag());
        }
        for ((key, english), (other, chinese)) in en::MESSAGES.iter().zip(zh::MESSAGES) {
            assert_eq!(key, other, "both catalogs list their keys in one order");
            assert_eq!(names(english), names(chinese), "{key}");
        }
        let chinese = catalogs(Language::SimplifiedChinese);
        assert_eq!(chinese.text("et.session.new", &[]).as_ref(), "新建会话");
    }

    #[test]
    fn terminal_catalogs_follow_host_locale_without_replacing_host_messages() {
        let host = I18n::new("zh-CN")
            .catalog("en-US", &[("host.title", "Host title")])
            .catalog("zh-CN", &[("host.title", "宿主标题")]);
        let terminal = for_locale(host.locale().tag);
        for (key, expected) in [
            ("et.status.waiting", "等待中"),
            ("et.status.done", "完成"),
            ("et.status.error", "错误"),
        ] {
            assert_eq!(terminal.text(key, &[]).as_ref(), expected);
        }
        assert_eq!(host.locale().tag, "zh-CN");
        assert_eq!(host.text("host.title", &[]).as_ref(), "宿主标题");

        let english = I18n::new("en-US")
            .catalog("en-US", &[("host.title", "Custom host title")]);
        let terminal = for_locale(english.locale().tag);
        for (key, expected) in [
            ("et.status.waiting", "Waiting"),
            ("et.status.done", "Done"),
            ("et.status.error", "Error"),
        ] {
            assert_eq!(terminal.text(key, &[]).as_ref(), expected);
        }
        assert_eq!(english.text("host.title", &[]).as_ref(), "Custom host title");
        assert_eq!(for_locale(host.locale().tag).text("et.status.done", &[]).as_ref(), "完成");
    }

    #[test]
    fn host_locale_formats_all_terminal_notification_states() {
        use crate::term::{Kind, Record, Records, State};

        let records = Records::default();
        for (tag, expected) in [
            ("en-US", ["Waiting", "Waiting for permission", "Waiting for an answer", "Waiting for a login", "Done", "Error"]),
            ("zh-CN", ["等待中", "等待授权", "等待回答", "等待登录", "完成", "错误"]),
        ] {
            let host = I18n::new(tag)
                .catalog("en-US", &[("host.title", "Host")])
                .catalog("zh-CN", &[("host.title", "宿主")]);
            for ((state, kind), expected) in [
                (State::Blocked, None),
                (State::Blocked, Some(Kind::Permission)),
                (State::Blocked, Some(Kind::Question)),
                (State::Blocked, Some(Kind::Auth)),
                (State::Done, None),
                (State::Error, None),
            ].into_iter().zip(expected) {
                let record = Record {
                    state,
                    kind,
                    progress: None,
                    app: None,
                    title: None,
                    msg: None,
                };
                assert_eq!(crate::sessions::line(&records, "", &record, for_locale(host.locale().tag)), expected);
            }
            assert_eq!(host.locale().tag, tag);
            assert_eq!(host.text("host.title", &[]).as_ref(), if tag == "zh-CN" { "宿主" } else { "Host" });
        }
    }

    #[test]
    fn every_key_the_source_names_is_in_the_catalogs_and_every_key_is_named() {
        let mut keys = BTreeSet::new();
        named(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut keys,
        );
        let listed: BTreeSet<String> = en::MESSAGES
            .iter()
            .map(|(key, _)| key.to_string())
            .collect();
        assert_eq!(keys, listed);
    }
}
