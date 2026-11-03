//! A small JSON writer: unc-asm has no dependencies beyond unc-core, and the language data,
//! analysis and tokens only need writing, never reading.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(&'static str, Json)>),
}

impl From<&str> for Json {
    fn from(s: &str) -> Json {
        Json::Str(s.to_string())
    }
}

impl From<String> for Json {
    fn from(s: String) -> Json {
        Json::Str(s)
    }
}

impl From<bool> for Json {
    fn from(b: bool) -> Json {
        Json::Bool(b)
    }
}

macro_rules! num {
    ($($t:ty),*) => {$(
        impl From<$t> for Json {
            fn from(n: $t) -> Json {
                Json::Num(n as i64)
            }
        }
    )*};
}
num!(u8, u16, u32, u64, usize, i64);

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(o: Option<T>) -> Json {
        o.map_or(Json::Null, Into::into)
    }
}

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(v: Vec<T>) -> Json {
        Json::Arr(v.into_iter().map(Into::into).collect())
    }
}

pub fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

impl Json {
    /// Compact JSON.
    pub fn compact(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, None, 0);
        s
    }

    /// JSON indented by two spaces, with arrays of plain values kept on one line.
    pub fn pretty(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, Some(2), 0);
        s.push('\n');
        s
    }

    fn flat(&self) -> bool {
        match self {
            Json::Arr(v) => v.iter().all(|j| !matches!(j, Json::Arr(_) | Json::Obj(_))),
            Json::Obj(_) => false,
            _ => true,
        }
    }

    fn write(&self, out: &mut String, indent: Option<usize>, depth: usize) {
        let nl = |out: &mut String, d: usize| {
            if let Some(n) = indent {
                out.push('\n');
                out.push_str(&" ".repeat(n * d));
            }
        };
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => out.push_str(&n.to_string()),
            Json::Str(s) => escape(s, out),
            Json::Arr(v) if v.is_empty() => out.push_str("[]"),
            Json::Arr(v) => {
                let inline = indent.is_none() || self.flat();
                out.push('[');
                for (i, j) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                        if inline && indent.is_some() {
                            out.push(' ');
                        }
                    }
                    if !inline {
                        nl(out, depth + 1);
                    }
                    j.write(out, indent, depth + 1);
                }
                if !inline {
                    nl(out, depth);
                }
                out.push(']');
            }
            Json::Obj(v) if v.is_empty() => out.push_str("{}"),
            Json::Obj(v) => {
                out.push('{');
                for (i, (k, j)) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    nl(out, depth + 1);
                    escape(k, out);
                    out.push(':');
                    if indent.is_some() {
                        out.push(' ');
                    }
                    j.write(out, indent, depth + 1);
                }
                nl(out, depth);
                out.push('}');
            }
        }
    }
}

/// Builds a `Json::Obj` from `key => value` pairs, converting each value with `Into<Json>`.
macro_rules! obj {
    ($($k:literal => $v:expr),* $(,)?) => {
        $crate::json::Json::Obj(vec![$(($k, $crate::json::Json::from($v))),*])
    };
}
pub(crate) use obj;

/// Character (Unicode scalar) offset of byte offset `byte` in `s`: what editors that index
/// strings by character (Godot, and JavaScript for everything outside the astral planes) use.
pub fn char_col(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().count()
}
