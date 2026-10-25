// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generates the example GPX files shipped with a release.
//!
//! The examples are produced from the real catalogue, so they are guaranteed to
//! be valid, to reference real stations and to stay in sync with the parser the
//! application uses. They are written to `examples/` and committed, so that a
//! user can open one without building anything.

use std::path::Path;

use stamp_core::{Stamp, StampCatalog, StampId};

use crate::workspace_root;

/// One example tour: a name, a description and the stations it visits.
struct Example {
    file: &'static str,
    name: &'static str,
    description: &'static str,
    stations: &'static [u16],
}

const EXAMPLES: &[Example] = &[
    Example {
        file: "brocken-runde.gpx",
        name: "Brocken-Runde (Beispiel)",
        description: "Beispieltour im Oberharz: sechs Stempelstellen rund um den Brocken. \
                      Erzeugt aus dem Stempelkatalog; die Wegpunkte sind echt, die Reihenfolge \
                      ist ein Vorschlag und keine geprüfte Route. Startpunkt ist nicht enthalten.",
        // Brockenhaus, Zeterklippe, Eckerloch, Achtermannshöhe, Gelber Brink,
        // Ahrentsklint — alle im Umkreis weniger Kilometer um den Brocken.
        stations: &[9, 10, 11, 12, 22, 13],
    },
    Example {
        file: "ilsenburg-runde.gpx",
        name: "Ilsenburg und Ilsestein (Beispiel)",
        description: "Beispieltour im Nordharz mit Stempelstellen um Ilsenburg. Wegpunkte echt, \
                      Reihenfolge unverbindlich. Startpunkt ist nicht enthalten.",
        // Eckertalsperre, Scharfenstein, Am Kruzifix, Taubenklippe,
        // Froschfelsen, Ilsestein.
        stations: &[1, 2, 3, 4, 5, 30],
    },
];

/// Writes all example files, returning the directory used.
pub fn generate() -> Result<std::path::PathBuf, String> {
    let root = workspace_root();
    let directory = root.join("examples");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;

    let catalog = StampCatalog::embedded();

    for example in EXAMPLES {
        let mut document = gpx::GpxDocument::new(example.name);
        document.description = Some(example.description.to_string());

        let mut positions = Vec::new();
        for number in example.stations {
            let stamp = station(catalog, *number)?;
            document.push_waypoint(waypoint(&stamp));
            positions.push(stamp.position);
        }

        // A straight-line "track" is intentionally not written: an example file
        // that looks like a hiking route but is a chain of straight lines would
        // teach exactly the wrong lesson.
        let xml = gpx::write_gpx(
            &document,
            &gpx::WriteOptions {
                include_waypoints: true,
                include_track: false,
                ..gpx::WriteOptions::default()
            },
        )
        .map_err(|error| format!("could not write {}: {error}", example.file))?;

        let path = directory.join(example.file);
        std::fs::write(&path, xml)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        println!(
            "wrote {} ({} stations)",
            relative(&root, &path),
            example.stations.len()
        );
    }

    Ok(directory)
}

fn station(catalog: &StampCatalog, number: u16) -> Result<Stamp, String> {
    let id = StampId::new(number).map_err(|error| error.to_string())?;
    catalog
        .get(id)
        .cloned()
        .ok_or_else(|| format!("station HWN{number:03} is not in the catalogue"))
}

fn waypoint(stamp: &Stamp) -> gpx::GpxWaypoint {
    let mut waypoint =
        gpx::GpxWaypoint::new(format!("{} {}", stamp.code, stamp.name), stamp.position);
    waypoint.elevation_m = stamp.elevation_m;
    waypoint.comment = Some(stamp.location.clone());
    waypoint.symbol = Some("Flag, Blue".to_string());
    waypoint
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
