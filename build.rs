use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=static/catpaw.css");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=static/assets/catpaw.min.css");

    let input = Path::new("static/catpaw.css");
    let output = Path::new("static/assets/catpaw.min.css");

    let Ok(raw) = fs::read_to_string(input) else {
        eprintln!("build: missing {}", input.display());
        return;
    };

    let minified = minify_css(&raw);
    if let Some(parent) = output.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!("build: failed to create {}: {err}", parent.display());
            return;
        }
    }

    if let Ok(existing) = fs::read_to_string(output) {
        if existing == minified {
            return;
        }
    }

    if let Err(err) = fs::write(output, minified) {
        eprintln!("build: failed to write {}: {err}", output.display());
    }
}

fn minify_css(input: &str) -> String {
    let without_comments = strip_comments(input);
    let collapsed = collapse_whitespace(&without_comments);
    let trimmed = trim_around_punctuation(&collapsed);
    trimmed.replace(";}", "}")
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string: Option<char> = None;
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if let Some(quote) = in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\x5c' {
                escape = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_string = Some(ch);
            out.push(ch);
            continue;
        }

        if ch == '/' && matches!(chars.peek(), Some('*')) {
            chars.next();
            while let Some(next) = chars.next() {
                if next == '*' && matches!(chars.peek(), Some('/')) {
                    chars.next();
                    break;
                }
            }
            continue;
        }

        out.push(ch);
    }

    out
}

fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string: Option<char> = None;
    let mut escape = false;
    let mut prev_space = false;

    for ch in input.chars() {
        if let Some(quote) = in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\x5c' {
                escape = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_string = Some(ch);
            out.push(ch);
            continue;
        }

        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
            continue;
        }

        prev_space = false;
        out.push(ch);
    }

    out.trim().to_string()
}

fn trim_around_punctuation(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string: Option<char> = None;
    let mut escape = false;
    let mut skip_space = false;

    for ch in input.chars() {
        if let Some(quote) = in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\x5c' {
                escape = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_string = Some(ch);
            out.push(ch);
            continue;
        }

        if skip_space && ch == ' ' {
            continue;
        }
        skip_space = false;

        if matches!(ch, '{' | '}' | ':' | ';' | ',' | '>' | '+' | '~' | '(' | ')' | '[' | ']') {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(ch);
            skip_space = true;
        } else {
            out.push(ch);
        }
    }

    out
}
