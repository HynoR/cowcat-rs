use std::path::{Component, Path};

use rust_embed::RustEmbed;
use base64::Engine;

#[derive(RustEmbed)]
#[folder = "static/"]
struct EmbeddedAssets;

pub fn load_template_assets() -> anyhow::Result<(String, String, String)> {
    let cowcat1 = EmbeddedAssets::get("assets/cowcat1.webp")
        .ok_or_else(|| anyhow::anyhow!("missing assets/cowcat1.webp"))?;
    let cowcat2 = EmbeddedAssets::get("assets/cowcat2.webp")
        .ok_or_else(|| anyhow::anyhow!("missing assets/cowcat2.webp"))?;
    let template_raw = EmbeddedAssets::get("catpaw.html")
        .ok_or_else(|| anyhow::anyhow!("missing catpaw.html"))?;

    let img1 = base64::engine::general_purpose::STANDARD.encode(cowcat1.data);
    let img2 = base64::engine::general_purpose::STANDARD.encode(cowcat2.data);
    let template = normalize_template(std::str::from_utf8(&template_raw.data)?);

    Ok((template, img1, img2))
}

pub fn get_asset(path: &str) -> Option<Vec<u8>> {
    let normalized = sanitize_path(path)?;
    EmbeddedAssets::get(&normalized).map(|data| data.data.into_owned())
}

fn normalize_template(raw: &str) -> String {
    raw.replace("{{.TaskData}}", "{{ TaskData }}")
        .replace("{{.RedirectURL}}", "{{ RedirectURL }}")
        .replace("{{.CowcatImage1}}", "{{ CowcatImage1 }}")
        .replace("{{.CowcatImage2}}", "{{ CowcatImage2 }}")
}

fn sanitize_path(path: &str) -> Option<String> {
    let trimmed = path.trim_start_matches('/');
    let mut clean = Vec::new();
    for comp in Path::new(trimmed).components() {
        match comp {
            Component::Normal(seg) => clean.push(seg),
            _ => return None,
        }
    }
    let mut out = String::new();
    for (idx, seg) in clean.iter().enumerate() {
        if idx > 0 {
            out.push('/');
        }
        out.push_str(seg.to_str()?);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
