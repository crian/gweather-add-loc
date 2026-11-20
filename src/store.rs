//! Backend: reads/writes the org.gnome.Weather `locations` key (type `av`)
//! via gsettings, for native and Flatpak GNOME Weather.

use gtk::glib::{prelude::ToVariant, Variant, VariantTy};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

pub const WEATHER_ID: &str = "org.gnome.Weather";

#[derive(Clone, Default)]
pub struct Store;

// Cache for flatpak detection result (avoids repeated `flatpak list` calls)
static FLATPAK_CACHE: OnceLock<bool> = OnceLock::new();

fn is_flatpak_install() -> bool {
    *FLATPAK_CACHE.get_or_init(|| {
        Command::new("flatpak")
            .args(["list", "--app", "--columns=application"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.split_whitespace().any(|l| l == WEATHER_ID))
            .unwrap_or(false)
    })
}

impl Store {
    pub fn new() -> Self {
        Self
    }

    /// Detected lazily so the first call does not block at startup.
    fn flatpak(&self) -> bool {
        is_flatpak_install()
    }

    fn run(&self, args: &[&str]) -> Result<std::process::Output, String> {
        let mut cmd = if self.flatpak() {
            let mut c = Command::new("flatpak");
            c.args(["run", "--command=gsettings", WEATHER_ID]);
            c
        } else {
            Command::new("gsettings")
        };
        cmd.args(args).output().map_err(|e| e.to_string())
    }

    /// Returns the `av` children (each a Variant of type `v`).
    pub fn entries(&self) -> Result<Vec<Variant>, String> {
        let out = self.run(&["get", WEATHER_ID, "locations"])?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() {
                "gsettings get failed".into()
            } else {
                err
            });
        }
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.is_empty() || text == "[]" || text == "@av []" {
            return Ok(Vec::new());
        }
        let ty = VariantTy::new("av").map_err(|e| e.to_string())?;
        let v = Variant::parse(Some(ty), &text).map_err(|e| e.to_string())?;
        Ok((0..v.n_children()).map(|i| v.child_value(i)).collect())
    }

    pub fn set_entries(&self, entries: &[Variant]) -> Result<(), String> {
        let arr = Variant::array_from_iter_with_type(VariantTy::VARIANT, entries.iter());
        let text = arr.print(false).to_string();
        let out = self.run(&["set", WEATHER_ID, "locations", &text])?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() {
                "gsettings set failed".into()
            } else {
                err
            });
        }
        Ok(())
    }

    /// Read-modify-write update with a bounded retry loop.
    ///
    /// Calls `f` with the current entries and writes back the returned ones.
    /// GSettings has no compare-and-swap, so concurrent writers can still
    /// overwrite each other; retries only help with transient write failures
    /// and are skipped when the stored value did not change.
    pub fn update_entries<F>(&self, mut f: F) -> Result<(), String>
    where
        F: FnMut(&[Variant]) -> Result<Vec<Variant>, String>,
    {
        const MAX_ATTEMPTS: u32 = 5;
        for attempt in 1..=MAX_ATTEMPTS {
            let current = self.entries()?;
            let new_entries = f(&current)?;
            match self.set_entries(&new_entries) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // A persistent error does not become a transient one by
                    // retrying, so only retry if the value changed meanwhile.
                    let unchanged = self.entries().is_ok_and(|after| after == current);
                    if attempt == MAX_ATTEMPTS || unchanged {
                        return Err(e);
                    }
                    std::thread::sleep(Duration::from_millis(50 * attempt as u64));
                }
            }
        }
        Err("update_entries: max attempts reached".into())
    }

    pub fn build_entry(name: &str, lat_deg: f64, lon_deg: f64) -> Variant {
        let coords = vec![(lat_deg.to_radians(), lon_deg.to_radians())];
        let inner = (
            name.to_string(),
            String::new(),
            false,
            coords,
            Vec::<(f64, f64)>::new(),
        )
            .to_variant();
        // libgweather-4 stores `(uint32, v)`, boxed as a `v` inside `av`.
        // `Variant` implements `ToVariant` by boxing, so no explicit
        // `from_variant` is needed for the inner value.
        let entry = (2u32, inner).to_variant();
        Variant::from_variant(&entry)
    }

    /// (name, lat_deg, lon_deg) of an entry, best effort.
    pub fn describe(entry: &Variant) -> (String, Option<(f64, f64)>) {
        // Expected: (uint32, (string, string, bool, a(dd), a(dd)))
        let outer = match entry.as_variant() {
            Some(v) => v,
            None => return ("(unknown entry)".to_string(), None),
        };
        if outer.n_children() != 2 {
            return ("(unknown entry)".to_string(), None);
        }

        let inner = match outer.child_value(1).as_variant() {
            Some(v) => v,
            None => return ("(unknown entry)".to_string(), None),
        };
        if inner.n_children() != 5 {
            return ("(unknown entry)".to_string(), None);
        }

        let name = inner
            .child_value(0)
            .str()
            .map(|s| s.to_string())
            .unwrap_or("(unnamed)".to_string());

        let coords = inner.child_value(3);
        let coords_deg = (coords.n_children() > 0)
            .then(|| coords.child_value(0))
            .filter(|tuple| tuple.n_children() == 2)
            .and_then(|tuple| {
                let lat = tuple.child_value(0).get::<f64>()?.to_degrees();
                let lon = tuple.child_value(1).get::<f64>()?.to_degrees();
                Some((lat, lon))
            });

        (name, coords_deg)
    }

    pub fn restart_weather(&self) {
        let mut cmd = if self.flatpak() {
            let _ = Command::new("flatpak").args(["kill", WEATHER_ID]).output();
            let mut c = Command::new("flatpak");
            c.args(["run", WEATHER_ID]);
            c
        } else {
            // Use -x for exact process name match (avoid killing unrelated processes)
            let _ = Command::new("pkill").args(["-x", "gnome-weather"]).output();
            Command::new("gnome-weather")
        };
        std::thread::sleep(Duration::from_millis(500));
        let _ = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_round_trips_built_entry() {
        let entry = Store::build_entry("Berlin", 52.52, 13.405);
        let (name, coords) = Store::describe(&entry);
        assert_eq!(name, "Berlin");
        let (lat, lon) = coords.expect("coordinates");
        assert!((lat - 52.52).abs() < 1e-6);
        assert!((lon - 13.405).abs() < 1e-6);
    }

    #[test]
    fn build_entry_escapes_but_keeps_names() {
        let name = "O'Brien \"x\" & <y> Zürich";
        let entry = Store::build_entry(name, 0.0, 0.0);
        assert_eq!(Store::describe(&entry).0, name);
    }
}
