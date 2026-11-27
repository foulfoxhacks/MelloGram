//! Prints the built-in wallpaper presets with their compiled CSS as JSON.
//! cargo run -p mellogram-core --example wallpaper_presets
fn main() {
    let out: Vec<_> = mellogram_core::wallpaper::presets()
        .into_iter()
        .map(|p| serde_json::json!({ "name": p.name, "css": p.wallpaper.layer_css(&|_| None), "wallpaper": p.wallpaper }))
        .collect();
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
