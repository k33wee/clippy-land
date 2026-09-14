mod app;
mod i18n;
mod ipc;
mod services;
mod settings;

fn main() -> cosmic::iced::Result {
    let mut open_popup_on_start = std::env::var_os("COSMIC_PANEL_NAME").is_none();

    for arg in std::env::args().skip(1) {
        if arg == "--toggle" || arg == "-t" {
            if let Err(e) = ipc::send_toggle() {
                eprintln!("Failed to toggle clippy-land: {e}");
                std::process::exit(1);
            }
            return Ok(());
        }

        if arg == "--standalone" {
            open_popup_on_start = true;
            continue;
        }

        if arg == "--no-standalone" {
            open_popup_on_start = false;
            continue;
        }

        if arg == "-h" || arg == "--help" {
            println!("Clippy Land - Clipboard history applet for COSMIC");
            println!();
            println!("USAGE:");
            println!("    cosmic-applet-clippy-land [OPTIONS]");
            println!();
            println!("OPTIONS:");
            println!("    -t, --toggle    Toggle the clipboard popup via keyboard shortcut");
            println!("    --standalone    Open popup window immediately on startup");
            println!("    --no-standalone Do not auto-open popup on startup");
            println!("    -h, --help      Print this help message");
            println!();
            println!("KEYBOARD SHORTCUT SETUP:");
            println!("    1. Open COSMIC Settings > Keyboard > Custom Shortcuts");
            println!("    2. Click 'Add Custom Shortcut'");
            println!("    3. Name:    Clipboard History");
            println!("    4. Command: cosmic-applet-clippy-land --toggle");
            println!("    5. Shortcut: Press Super+V (or your preferred shortcut)");
            return Ok(());
        }
    }

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);
    cosmic::applet::run::<app::AppModel>(app::AppFlags {
        open_popup_on_start,
    })
}
