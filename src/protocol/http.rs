use axum::http::{header, HeaderMap};

pub trait HeaderMapExt {
    fn get_str<N>(&self, name: N) -> Option<&str>
    where
        N: header::AsHeaderName;

    fn get_string<N>(&self, name: N) -> Option<String>
    where
        N: header::AsHeaderName;

    fn get_string_or_default<N>(&self, name: N) -> String
    where
        N: header::AsHeaderName;

    fn get_ip<N>(&self, name: N) -> Option<String>
    where
        N: header::AsHeaderName;
}

impl HeaderMapExt for HeaderMap {
    fn get_str<N>(&self, name: N) -> Option<&str>
    where
        N: header::AsHeaderName,
    {
        self.get(name).and_then(|value| value.to_str().ok())
    }

    fn get_string<N>(&self, name: N) -> Option<String>
    where
        N: header::AsHeaderName,
    {
        self.get_str(name).map(|value| value.to_string())
    }

    fn get_string_or_default<N>(&self, name: N) -> String
    where
        N: header::AsHeaderName,
    {
        self.get_string(name).unwrap_or_default()
    }

    fn get_ip<N>(&self, name: N) -> Option<String>
    where
        N: header::AsHeaderName,
    {
        let value = self.get_str(name)?;
        let first = value.split(',').next()?.trim();
        if first.is_empty() {
            None
        } else {
            Some(first.to_string())
        }
    }
}
