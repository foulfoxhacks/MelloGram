//! Prints the built-in plugin catalog as JSON (`--markdown` for a table).
//!
//! cargo run -p mellogram-plugins --example catalog -- --markdown

use mellogram_core::{PluginManager, Settings};

fn main() {
    let mut manager = PluginManager::new(mellogram_plugins::builtin(), None);
    let settings = Settings::default();
    manager.init(&settings);
    let mut list = manager.list(&settings);
    list.sort_by(|a, b| a.manifest.name.cmp(b.manifest.name));
    if std::env::args().any(|a| a == "--markdown") {
        println!("| Plugin | Category | Default | Adapts | Description |\n|---|---|---|---|---|");
        for p in &list {
            let m = &p.manifest;
            println!(
                "| **{}** (`{}`) | {:?} | {} | {} | {} |",
                m.name,
                m.id,
                m.category,
                if p.enabled { "on" } else { "off" },
                m.inspired_by.unwrap_or("—"),
                m.description
            );
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
    }
}
