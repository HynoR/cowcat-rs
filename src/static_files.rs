use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path};

use base64::Engine;
use bytes::Bytes;
use rust_embed::RustEmbed;

use crate::config::PowPageConfig;

const CORE_JS_TAG: &str = r#"<script data-cfasync="false" src="/__cowcatwaf/assets/catpaw.core.js"></script>"#;
const STYLE_JS_TAG: &str = r#"<script data-cfasync="false" src="/__cowcatwaf/assets/catpaw.style.js"></script>"#;

#[derive(RustEmbed)]
#[folder = "static/"]
struct EmbeddedAssets;

pub struct TemplateAssets {
    pub template: String,
    pub cowcat_image1: String,
    pub cowcat_image2: String,
    pub assets: HashMap<String, Bytes>,
}

struct EmbeddedDefaults {
    template_raw: String,
    css: String,
    cowcat_image1: String,
    cowcat_image2: String,
    meta_js: String,
    assets: HashMap<String, Bytes>,
}

pub fn load_template_assets(page: &PowPageConfig) -> anyhow::Result<TemplateAssets> {
    let defaults = load_embedded_defaults()?;
    let default_template = build_template(&defaults.template_raw, true, &defaults.css, &defaults.meta_js, page.enable_meta, true);
    let default_assets = TemplateAssets {
        template: default_template,
        cowcat_image1: defaults.cowcat_image1.clone(),
        cowcat_image2: defaults.cowcat_image2.clone(),
        assets: defaults.assets.clone(),
    };

    if !page.custom {
        return Ok(default_assets);
    }

    match load_custom_assets(page, &defaults) {
        Ok(custom_assets) => Ok(custom_assets),
        Err(err) => {
            tracing::error!(error = %err, "custom pow page invalid, using default");
            Ok(default_assets)
        }
    }
}

fn load_custom_assets(page: &PowPageConfig, defaults: &EmbeddedDefaults) -> anyhow::Result<TemplateAssets> {
    let html_path = page.html.trim();
    if html_path.is_empty() {
        anyhow::bail!("pow.page.custom enabled but pow.page.html is empty");
    }

    let html_raw = fs::read_to_string(html_path)
        .map_err(|err| anyhow::anyhow!("failed to read custom html {html_path}: {err}"))?;

    let style_js_path = page.style_js.trim();
    let style_js = if style_js_path.is_empty() {
        None
    } else {
        Some(
            fs::read(style_js_path)
                .map_err(|err| anyhow::anyhow!("failed to read custom style js {style_js_path}: {err}"))?,
        )
    };

    let style_css_path = page.style_css.trim();
    let style_css = if style_css_path.is_empty() {
        None
    } else {
        Some(
            fs::read_to_string(style_css_path)
                .map_err(|err| anyhow::anyhow!("failed to read custom style css {style_css_path}: {err}"))?,
        )
    };

    validate_custom_template(&html_raw, style_js.is_some(), style_css.is_some())?;

    let css_content = style_css.unwrap_or_else(|| defaults.css.clone());
    let template = build_template(&html_raw, style_js.is_some(), &css_content, &defaults.meta_js, page.enable_meta, false);

    let mut assets = defaults.assets.clone();
    if let Some(js) = style_js {
        assets.insert("assets/catpaw.style.js".to_string(), Bytes::from(js));
    }

    Ok(TemplateAssets {
        template,
        cowcat_image1: defaults.cowcat_image1.clone(),
        cowcat_image2: defaults.cowcat_image2.clone(),
        assets,
    })
}



fn load_embedded_defaults() -> anyhow::Result<EmbeddedDefaults> {
    let mut assets = HashMap::new();

    let meta_js_bytes = load_minify_or_embedded("core_minify/meta.min.js", "core/meta.js")?;
    let meta_js = String::from_utf8(meta_js_bytes.to_vec())
        .map_err(|err| anyhow::anyhow!("meta.js is not valid UTF-8: {err}"))?;

    let core_js = load_minify_or_embedded("core_minify/catpaw.core.min.js", "core/catpaw.core.js")?;
    assets.insert("assets/catpaw.core.js".to_string(), core_js);

    let worker_js = load_minify_or_embedded("core_minify/catpaw.worker.min.js", "core/catpaw.worker.js")?;
    assets.insert("assets/catpaw.worker.js".to_string(), worker_js);

    let wasm = load_embedded_bytes("core/catpaw.wasm")?;
    assets.insert("assets/catpaw.wasm".to_string(), wasm);

    let style_js = load_embedded_bytes("default/catpaw.style.js")?;
    assets.insert("assets/catpaw.style.js".to_string(), style_js);

    let template_raw = load_embedded_string("default/catpaw.html")?;
    let css = load_embedded_string("core_minify/catpaw.min.css").or_else(|_| load_embedded_string("default/catpaw.css"))?;

    let cowcat1 = load_embedded_bytes("default/cowcat1.webp")?;
    let cowcat2 = load_embedded_bytes("default/cowcat2.webp")?;

    let cowcat_image1 = base64::engine::general_purpose::STANDARD.encode(cowcat1.as_ref());
    let cowcat_image2 = base64::engine::general_purpose::STANDARD.encode(cowcat2.as_ref());

    Ok(EmbeddedDefaults {
        template_raw,
        css,
        cowcat_image1,
        cowcat_image2,
        meta_js,
        assets,
    })
}

fn load_embedded_bytes(path: &str) -> anyhow::Result<Bytes> {
    let data = EmbeddedAssets::get(path)
        .ok_or_else(|| anyhow::anyhow!("missing {path}"))?;
    Ok(match data.data {
        std::borrow::Cow::Borrowed(bytes) => Bytes::from_static(bytes),
        std::borrow::Cow::Owned(vec) => Bytes::from(vec),
    })
}


// 优先读取 minify_path 文件，如果有，则读取 minify_path 文件，否则读取 base_path 文件
fn load_minify_or_embedded(minify_path: &str, base_path: &str) -> anyhow::Result<Bytes> {
    let data = load_embedded_bytes(minify_path).or_else(|_| load_embedded_bytes(base_path))?;
    Ok(data)
}


fn load_embedded_string(path: &str) -> anyhow::Result<String> {
    let data = EmbeddedAssets::get(path)
        .ok_or_else(|| anyhow::anyhow!("missing {path}"))?;
    let text = std::str::from_utf8(&data.data)?;
    Ok(text.to_string())
}

fn build_template(normalized: &str, style_js_enabled: bool, css: &str, meta_js: &str, enable_meta: bool, minify: bool) -> String {
    let mut template = if minify {
        minify_template_lines(normalized)
    } else {
        normalized.to_string()
    };

    if enable_meta {
        let meta_js_inline = format!(r#"<script data-cfasync="false">{}</script>"#, meta_js);
        template = template.replace("{{ MetaJS }}", &meta_js_inline);
    } else {
        template = template.replace("{{ MetaJS }}", "");
    }
    template = template.replace("{{ CoreJS }}", CORE_JS_TAG);
    if style_js_enabled {
        template = template.replace("{{ StyleJS }}", STYLE_JS_TAG);
    } else {
        template = template.replace("{{ StyleJS }}", "");
    }
    template.replace("{{ CatpawCSS }}", css)
}

fn validate_custom_template(
    raw: &str,
    needs_style_js: bool,
    needs_style_css: bool,
) -> anyhow::Result<()> {
    if !raw.contains("{{ TaskData }}") {
        anyhow::bail!("custom html missing {{ TaskData }} placeholder");
    }
    if !raw.contains("{{ RedirectURL }}") {
        anyhow::bail!("custom html missing {{ RedirectURL }} placeholder");
    }

    let has_core = raw.contains("{{ CoreJS }}") || has_script_reference(raw, "catpaw.core.js");
    if !has_core {
        anyhow::bail!("custom html missing {{ CoreJS }} or catpaw.core.js script tag");
    }

    if needs_style_js {
        let has_style = raw.contains("{{ StyleJS }}") || has_script_reference(raw, "catpaw.style.js");
        if !has_style {
            anyhow::bail!("custom html missing {{ StyleJS }} or catpaw.style.js script tag");
        }
    }

    if needs_style_css && !raw.contains("{{ CatpawCSS }}") {
        anyhow::bail!("custom html missing {{ CatpawCSS }} placeholder");
    }

    Ok(())
}

fn has_script_reference(raw: &str, filename: &str) -> bool {
    raw.contains(filename)
}

fn minify_template_lines(raw: &str) -> String {
    raw.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("")
}

pub fn sanitize_path(path: &str) -> Option<String> {
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
