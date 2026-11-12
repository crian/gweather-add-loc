mod store;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};

const APP_ID: &str = "io.github.crian.GWeatherAddLoc";
const CHANGELOG: &str = include_str!("../CHANGELOG.md");

/// Extract the changelog section for `version` and convert it to the AppStream
/// markup expected by `AdwAboutWindow::release_notes`.
fn release_notes(version: &str) -> Option<String> {
    let heading = format!("## {version}");
    let mut lines = CHANGELOG.lines();
    lines.find(|line| {
        line.trim_start()
            .strip_prefix(&heading)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })?;

    let mut html = String::new();
    let mut list_open = false;
    let close_list = |html: &mut String, open: &mut bool| {
        if *open {
            html.push_str("</ul>");
            *open = false;
        }
    };

    for line in lines.take_while(|line| !line.starts_with("## ")) {
        let line = line.trim();
        if line.is_empty() {
            close_list(&mut html, &mut list_open);
        } else if let Some(group) = line.strip_prefix("### ") {
            close_list(&mut html, &mut list_open);
            html.push_str("<p><em>");
            html.push_str(&inline_markup(group));
            html.push_str("</em></p>");
        } else if let Some(item) = line.strip_prefix("- ") {
            if !list_open {
                html.push_str("<ul>");
                list_open = true;
            }
            html.push_str("<li>");
            html.push_str(&inline_markup(item));
            html.push_str("</li>");
        } else {
            close_list(&mut html, &mut list_open);
            html.push_str("<p>");
            html.push_str(&inline_markup(line));
            html.push_str("</p>");
        }
    }
    close_list(&mut html, &mut list_open);

    (!html.is_empty()).then_some(html)
}

/// Escape HTML and turn `**strong**` and `` `code` `` into the markup that
/// AppStream descriptions support.
fn inline_markup(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let code = replace_pairs(&escaped, "`", "code");
    replace_pairs(&code, "**", "em")
}

fn replace_pairs(input: &str, delimiter: &str, tag: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find(delimiter) {
        let after = &rest[start + delimiter.len()..];
        let Some(end) = after.find(delimiter) else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push('<');
        out.push_str(tag);
        out.push('>');
        out.push_str(&after[..end]);
        out.push_str("</");
        out.push_str(tag);
        out.push('>');
        rest = &after[end + delimiter.len()..];
    }
    out.push_str(rest);
    out
}

#[allow(deprecated)]
fn show_about(app: &adw::Application) {
    let version = env!("CARGO_PKG_VERSION");
    let mut builder = adw::AboutWindow::builder()
        .application_name("Weather Locations")
        .application_icon(APP_ID)
        .version(version)
        .developer_name("crian")
        .developers(["crian <crian84@gmail.com>"])
        .copyright("© 2026 crian")
        .comments("Add custom locations with arbitrary coordinates to GNOME Weather.")
        .website("https://github.com/crian/gweather-add-loc")
        .license_type(gtk::License::MitX11);
    if let Some(notes) = release_notes(version) {
        builder = builder.release_notes(&notes);
    }
    let about = builder.build();
    about.set_transient_for(app.active_window().as_ref());
    about.present();
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_startup(|app| {
        let quit = gio::ActionEntry::builder("quit")
            .activate(|app: &adw::Application, _, _| app.quit())
            .build();
        let about = gio::ActionEntry::builder("about")
            .activate(|app: &adw::Application, _, _| show_about(app))
            .build();
        app.add_action_entries([quit, about]);
        app.set_accels_for_action("app.quit", &["<primary>q"]);
        app.set_accels_for_action("win.reload", &["<primary>r"]);
    });

    app.connect_activate(|app| {
        if let Some(win) = app.active_window() {
            win.present();
        } else {
            window::build(app).present();
        }
    });

    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_notes_for_current_version() {
        let notes = release_notes(env!("CARGO_PKG_VERSION"))
            .expect("changelog has an entry for this version");
        assert!(notes.contains("<ul>"));
        assert!(notes.contains("<li>"));
        assert!(!notes.contains("## "));
        assert!(!notes.contains("**"));
    }

    #[test]
    fn release_notes_for_unknown_version() {
        assert!(release_notes("0.0.0-does-not-exist").is_none());
    }

    #[test]
    fn inline_markup_escapes_and_formats() {
        assert_eq!(
            inline_markup("a < b & **c** `d`"),
            "a &lt; b &amp; <em>c</em> <code>d</code>"
        );
        assert_eq!(inline_markup("unbalanced **text"), "unbalanced **text");
    }
}
