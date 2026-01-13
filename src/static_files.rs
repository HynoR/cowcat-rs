use std::path::{Component, Path};
use std::fs;

use bytes::Bytes;
use rust_embed::RustEmbed;
use base64::Engine;

#[derive(RustEmbed)]
#[folder = "static/"]
struct EmbeddedAssets;

pub fn load_template_assets(
    cowcat_image1_path: Option<&str>,
    cowcat_image2_path: Option<&str>,
) -> anyhow::Result<(String, String, String)> {
    let cowcat1 = load_cowcat_image(cowcat_image1_path, "assets/cowcat1.webp", "cowcat_image1_path")?;
    let cowcat2 = load_cowcat_image(cowcat_image2_path, "assets/cowcat2.webp", "cowcat_image2_path")?;
    let css_raw = EmbeddedAssets::get("assets/catpaw.min.css")
        .or_else(|| EmbeddedAssets::get("catpaw.css"))
        .ok_or_else(|| anyhow::anyhow!("missing catpaw css"))?;
    let template_raw = EmbeddedAssets::get("catpaw.html")
        .ok_or_else(|| anyhow::anyhow!("missing catpaw.html"))?;

    let img1 = base64::engine::general_purpose::STANDARD.encode(cowcat1);
    let img2 = base64::engine::general_purpose::STANDARD.encode(cowcat2);
    let template = std::str::from_utf8(&template_raw.data)?;
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

fn minify_template_lines(raw: &str) -> String {
    raw.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("")
}

fn load_cowcat_image(
    path: Option<&str>,
    embedded_path: &str,
    config_key: &str,
) -> anyhow::Result<Vec<u8>> {
    if let Some(raw_path) = path {
        let trimmed = raw_path.trim();
        if !trimmed.is_empty() {
            return fs::read(trimmed)
                .map_err(|err| anyhow::anyhow!("failed to read {config_key} {trimmed}: {err}"));
        }
    }
    let embedded = EmbeddedAssets::get(embedded_path)
        .ok_or_else(|| anyhow::anyhow!("missing {embedded_path}"))?;
    Ok(embedded.data.into_owned())
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
