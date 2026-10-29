use std::sync::LazyLock;

use i18n_embed::{
    DefaultLocalizer, LanguageLoader, Localizer,
    fluent::{FluentLanguageLoader, fluent_language_loader},
    unic_langid::LanguageIdentifier,
};
use rust_embed::RustEmbed;

pub fn init(requested_languages: &[LanguageIdentifier]) {
    if let Err(why) = localizer().select(requested_languages) {
        eprintln!("error while loading fluent localizations: {why}");
    }
}

#[must_use]
pub fn localizer() -> Box<dyn Localizer> {
    Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations))
}

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();

    loader
        .load_fallback_language(&Localizations)
        .expect("Error while loading fallback language");

    loader
});

/// Request a localized string by ID from the i18n/ directory
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id, $($args), *)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn strip_isolates(text: &str) -> String {
        text.chars()
            .filter(|c| !matches!(*c, '\u{2066}'..='\u{2069}'))
            .collect()
    }

    /// Italian is the only fully maintained locale besides English. Other
    /// locales are completed through the standard translation workflow, so
    /// this guard keeps `it` in sync with every English key.
    #[test]
    fn italian_translations_cover_every_english_message() {
        let en: LanguageIdentifier = "en".parse().unwrap();
        let it: LanguageIdentifier = "it".parse().unwrap();
        let loader = FluentLanguageLoader::new("cosmic_applet_clippy_land", en.clone());
        loader
            .load_languages(&Localizations, &[en.clone(), it.clone()])
            .expect("English and Italian bundles must load");
        let loader = loader.select_languages(std::slice::from_ref(&it));

        let english_keys = loader.with_message_iter(&en, |messages| {
            messages
                .map(|message| message.id.name.to_string())
                .collect::<Vec<_>>()
        });
        let italian_keys = loader.with_message_iter(&it, |messages| {
            messages
                .map(|message| message.id.name.to_string())
                .collect::<Vec<_>>()
        });
        assert!(!english_keys.is_empty(), "English messages must load");

        for key in &english_keys {
            assert!(
                italian_keys.contains(key),
                "missing Italian translation for `{key}`"
            );
            assert!(
                !loader.get(key).trim().is_empty(),
                "empty Italian value for `{key}`"
            );
        }

        // Spot-check translations, including Fluent argument formatting.
        assert_eq!(loader.get("settings"), "Impostazioni");
        assert_eq!(loader.get("empty"), "La clipboard è vuota");
        let mut args: HashMap<String, u32> = HashMap::new();
        args.insert("min".to_string(), 30);
        args.insert("max".to_string(), 5000);
        assert_eq!(
            strip_isolates(&loader.get_args("settings-error-history-range", args)),
            "Le voci in cronologia devono essere comprese tra 30 e 5000"
        );
    }
}
