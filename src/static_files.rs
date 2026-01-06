use std::path::{Component, Path};

use bytes::Bytes;
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
    let css_raw = EmbeddedAssets::get("assets/catpaw.min.css")
        .or_else(|| EmbeddedAssets::get("catpaw.css"))
        .ok_or_else(|| anyhow::anyhow!("missing catpaw css"))?;
    let template_raw = EmbeddedAssets::get("catpaw.html")
        .ok_or_else(|| anyhow::anyhow!("missing catpaw.html"))?;

    let img1 = base64::engine::general_purpose::STANDARD.encode(cowcat1.data);
    let img2 = base64::engine::general_purpose::STANDARD.encode(cowcat2.data);
    let template = normalize_template(std::str::from_utf8(&template_raw.data)?);
    let template = minify_template_lines(&template);
    let template = template.replace("{{ CatpawCSS }}", std::str::from_utf8(&css_raw.data)?);

    Ok((template, img1, img2))
}

pub fn get_asset(path: &str) -> Option<Bytes> {
    let normalized = sanitize_path(path)?;
    EmbeddedAssets::get(&normalized).map(|data| {
        match data.data {
            std::borrow::Cow::Borrowed(bytes) => Bytes::from_static(bytes),
            std::borrow::Cow::Owned(vec) => Bytes::from(vec),
        }
    })
}

fn normalize_template(raw: &str) -> String {
    raw.replace("{{.TaskData}}", "{{ TaskData }}")
        .replace("{{.RedirectURL}}", "{{ RedirectURL }}")
        .replace("{{.CowcatImage1}}", "{{ CowcatImage1 }}")
        .replace("{{.CowcatImage2}}", "{{ CowcatImage2 }}")
        .replace("{{.CatpawCSS}}", "{{ CatpawCSS }}")
}

fn minify_template_lines(raw: &str) -> String {
    raw.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("")
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
