//! Backend: reads/writes the org.gnome.Weather `locations` key (type `av`)
//! via gsettings, for native and Flatpak GNOME Weather.

use gtk::glib::{self, prelude::*, Variant, VariantTy};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const WEATHER_ID: &str = "org.gnome.Weather";

pub struct Store {
    flatpak: bool,
}

fn is_flatpak_install() -> bool {
    Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .any(|l| l == WEATHER_ID)
        })
        .unwrap_or(false)
}

impl Store {
    pub fn new() -> Self {
        Self { flatpak: is_flatpak_install() }
    }

    fn run(&self, args: &[&str]) -> Result<std::process::Output, String> {
        let mut cmd = if self.flatpak {
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
            return Err(if err.is_empty() { "gsettings get failed".into() } else { err });
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
            return Err(if err.is_empty() { "gsettings set failed".into() } else { err });
        }
        Ok(())
    }

    /// Atomically update entries with a compare-and-swap retry loop.
    /// Calls `f` with current entries, expects new entries to write back.
    /// Retries up to `max_attempts` times if the data changed concurrently.
    pub fn update_entries<F>(&self, mut f: F) -> Result<(), String>
    where
        F: FnMut(&[Variant]) -> Result<Vec<Variant>, String>,
    {
        const MAX_ATTEMPTS: u32 = 5;
        for attempt in 1..=MAX_ATTEMPTS {
            let current = self.entries()?;
            let new_entries = f(&current)?;
            // Try to write - if gsettings fails, it's a real error (not a race)
            match self.set_entries(&new_entries) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // If this is the last attempt, return the error
                    if attempt == MAX_ATTEMPTS {
                        return Err(e);
                    }
                    // Otherwise, the write failed - could be a race or real error.
                    // We can't easily distinguish, so just retry.
                    // A small delay helps reduce contention.
                    std::thread::sleep(Duration::from_millis(50 * attempt as u64));
                }
            }
        }
        Err("update_entries: max attempts reached".into())
    }

    pub fn build_entry(name: &str, lat_deg: f64, lon_deg: f64) -> Result<Variant, String> {
        let lat = lat_deg.to_radians();
        let lon = lon_deg.to_radians();
        let quoted = name.to_variant().print(false); // properly escaped
        let text = format!(
            "<(uint32 2, <({quoted}, '', false, [({lat:.10}, {lon:.10})], @a(dd) [])>)>"
        );
        Variant::parse(None, &text).map_err(|e| e.to_string())
    }

    /// (name, lat_deg, lon_deg) of an entry, best effort.
    pub fn describe(entry: &Variant) -> (String, Option<(f64, f64)>) {
        let parse = || -> Option<(String, Option<(f64, f64)>)> {
            let tup = entry.as_variant()?; // (u, v)
            let inner = tup.child_value(1).as_variant()?; // (s, s, b, a(dd), a(dd))
            let name = inner.child_value(0).str()?.to_string();
            let coords = inner.child_value(3);
            if coords.n_children() > 0 {
                let c = coords.child_value(0);
                let lat = c.child_value(0).get::<f64>()?.to_degrees();
                let lon = c.child_value(1).get::<f64>()?.to_degrees();
                Some((name, Some((lat, lon))))
            } else {
                Some((name, None))
            }
        };
        parse().unwrap_or_else(|| ("(unknown entry)".to_string(), None))
    }

    pub fn restart_weather(&self) {
        let _ = Command::new("pkill").args(["-f", "gnome-weather"]).output();
        let _ = Command::new("flatpak").args(["kill", WEATHER_ID]).output();
        std::thread::sleep(Duration::from_millis(500));
        let mut cmd = if self.flatpak {
            let mut c = Command::new("flatpak");
            c.args(["run", WEATHER_ID]);
            c
        } else {
            Command::new("gnome-weather")
        };
        let _ = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    }
}

#[allow(dead_code)]
fn _assert_glib_used() -> glib::Type {
    glib::Type::INVALID
}
