//! Main window.

use crate::store::Store;
use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

struct Ui {
    toasts: adw::ToastOverlay,
    name_row: adw::EntryRow,
    lat_row: adw::EntryRow,
    lon_row: adw::EntryRow,
    restart_row: adw::SwitchRow,
    list_group: adw::PreferencesGroup,
    rows: RefCell<Vec<adw::ActionRow>>,
    store: Store,
}

fn parse_float(text: &str) -> Result<f64, std::num::ParseFloatError> {
    // accept both "." and "," as decimal separator
    text.trim().replace(',', ".").parse::<f64>()
}

impl Ui {
    fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    fn reload(self: &Rc<Self>) {
        for row in self.rows.borrow_mut().drain(..) {
            self.list_group.remove(&row);
        }
        let entries = match self.store.entries() {
            Ok(e) => e,
            Err(e) => {
                self.toast(&format!("Error reading locations: {e}"));
                return;
            }
        };
        if entries.is_empty() {
            let row = adw::ActionRow::builder().title("No locations saved").build();
            self.list_group.add(&row);
            self.rows.borrow_mut().push(row);
            return;
        }
        for (idx, entry) in entries.iter().enumerate() {
            let (name, coords) = Store::describe(entry);
            let sub = coords
                .map(|(lat, lon)| format!("{lat:.4}, {lon:.4}"))
                .unwrap_or_default();
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&name))
                .subtitle(sub)
                .build();
            let btn = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .valign(gtk::Align::Center)
                .tooltip_text("Remove")
                .css_classes(["flat"])
                .build();
            let ui = self.clone();
            btn.connect_clicked(move |_| ui.on_remove(idx));
            row.add_suffix(&btn);
            self.list_group.add(&row);
            self.rows.borrow_mut().push(row);
        }
    }

    fn finish(self: &Rc<Self>, msg: &str) {
        if self.restart_row.is_active() {
            self.store.restart_weather();
        }
        self.toast(msg);
        self.reload();
    }

    fn on_add(self: &Rc<Self>) {
        let name = self.name_row.text().trim().to_string();
        if name.is_empty() {
            self.toast("Please enter a name");
            return;
        }
        let (lat, lon) = match (
            parse_float(&self.lat_row.text()),
            parse_float(&self.lon_row.text()),
        ) {
            (Ok(a), Ok(b)) => (a, b),
            _ => {
                self.toast("Coordinates must be numbers");
                return;
            }
        };
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            self.toast("Coordinates out of range");
            return;
        }
        let result = self.store.entries().and_then(|mut entries| {
            entries.insert(0, Store::build_entry(&name, lat, lon)?);
            self.store.set_entries(&entries)
        });
        if let Err(e) = result {
            self.toast(&format!("Error: {e}"));
            return;
        }
        self.name_row.set_text("");
        self.lat_row.set_text("");
        self.lon_row.set_text("");
        self.finish(&format!("Added \u{201c}{name}\u{201d}"));
    }

    fn on_remove(self: &Rc<Self>, idx: usize) {
        let result = self.store.entries().and_then(|mut entries| {
            if idx >= entries.len() {
                return Ok(None);
            }
            let name = Store::describe(&entries[idx]).0;
            entries.remove(idx);
            self.store.set_entries(&entries)?;
            Ok(Some(name))
        });
        match result {
            Ok(Some(name)) => self.finish(&format!("Removed \u{201c}{name}\u{201d}")),
            Ok(None) => {}
            Err(e) => self.toast(&format!("Error: {e}")),
        }
    }
}

pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    let toasts = adw::ToastOverlay::new();

    let name_row = adw::EntryRow::builder().title("Name").build();
    let lat_row = adw::EntryRow::builder().title("Latitude (−90 … 90)").build();
    let lon_row = adw::EntryRow::builder().title("Longitude (−180 … 180)").build();
    let restart_row = adw::SwitchRow::builder()
        .title("Restart GNOME Weather")
        .subtitle("Applies the change immediately")
        .active(true)
        .build();

    let add_group = adw::PreferencesGroup::builder()
        .title("New Location")
        .description("Coordinates in decimal degrees, e.g. 52.5200 / 13.4050")
        .build();
    for r in [&name_row, &lat_row, &lon_row] {
        add_group.add(r);
    }
    add_group.add(&restart_row);

    let add_btn = gtk::Button::builder()
        .label("Add Location")
        .halign(gtk::Align::Center)
        .css_classes(["suggested-action", "pill"])
        .build();
    let btn_group = adw::PreferencesGroup::new();
    btn_group.add(&add_btn);

    let list_group = adw::PreferencesGroup::builder().title("Saved Locations").build();

    let page = adw::PreferencesPage::new();
    page.add(&add_group);
    page.add(&btn_group);
    page.add(&list_group);

    let menu = gio::Menu::new();
    menu.append(Some("Reload"), Some("win.reload"));
    menu.append(Some("About Weather Locations"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    let header = adw::HeaderBar::new();
    header.pack_end(
        &gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .primary(true)
            .build(),
    );

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&page));
    toasts.set_child(Some(&view));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Weather Locations")
        .default_width(480)
        .default_height(660)
        .content(&toasts)
        .build();

    let ui = Rc::new(Ui {
        toasts,
        name_row,
        lat_row,
        lon_row,
        restart_row,
        list_group,
        rows: RefCell::new(Vec::new()),
        store: Store::new(),
    });

    for r in [&ui.name_row, &ui.lat_row, &ui.lon_row] {
        let u = ui.clone();
        r.connect_entry_activated(move |_| u.on_add());
    }
    {
        let u = ui.clone();
        add_btn.connect_clicked(move |_| u.on_add());
    }
    {
        let u = ui.clone();
        let reload = gio::ActionEntry::builder("reload")
            .activate(move |_: &adw::ApplicationWindow, _, _| u.reload())
            .build();
        window.add_action_entries([reload]);
    }

    ui.reload();
    window
}
