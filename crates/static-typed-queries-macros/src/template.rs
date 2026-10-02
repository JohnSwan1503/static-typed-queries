use syn::LitStr;

pub(crate) enum Segment {
    Lit(String),
}

pub(crate) struct Template {
    pub segments: Vec<Segment>,
}

pub(crate) fn parse(sql: &LitStr) -> syn::Result<Template> {
    let error = |message: String| syn::Error::new(sql.span(), message);
    let text = scan(&sql.value()).map_err(error)?;
    Ok(Template {
        segments: vec![Segment::Lit(text)],
    })
}

fn scan(text: &str) -> Result<String, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut lit = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match (c, next) {
            ('\'' | '"', _) => {
                let mut end = i + 1;
                loop {
                    match chars.get(end) {
                        None => return Err(format!("unterminated {c}…{c} in the SQL template")),
                        Some(&q) if q == c && chars.get(end + 1) == Some(&c) => end += 2,
                        Some(&q) if q == c => break,
                        Some(_) => end += 1,
                    }
                }
                lit.extend(&chars[i..=end]);
                i = end + 1;
            }
            ('$', _) if dollar_tag(&chars, i).is_some() => {
                let tag = dollar_tag(&chars, i).expect("a dollar quote tag");
                let body = i + tag.len();
                let end = (body..chars.len())
                    .find(|&j| chars[j..].starts_with(&tag))
                    .ok_or("unterminated $$ string in the SQL template")?;
                lit.extend(&chars[i..end + tag.len()]);
                i = end + tag.len();
            }
            ('-', Some('-')) => {
                i = (i..chars.len())
                    .find(|&j| chars[j] == '\n')
                    .unwrap_or(chars.len());
            }
            (c, _) if c.is_whitespace() => {
                if !lit.is_empty() && !lit.ends_with(' ') {
                    lit.push(' ');
                }
                i += 1;
            }
            ('/', Some('*')) => {
                let end = (i + 2..chars.len())
                    .find(|&j| chars[j] == '/' && chars[j - 1] == '*')
                    .ok_or("unterminated /* comment in the SQL template")?;
                lit.extend(&chars[i..=end]);
                i = end + 1;
            }
            _ => {
                lit.push(c);
                i += 1;
            }
        }
    }
    lit.truncate(lit.trim_end().len());
    Ok(lit)
}

fn dollar_tag(chars: &[char], start: usize) -> Option<Vec<char>> {
    let mut end = start + 1;
    while let Some(&c) = chars.get(end) {
        if c == '$' {
            return Some(chars[start..=end].to_vec());
        }
        let valid = c == '_' || c.is_alphabetic() || (end > start + 1 && c.is_ascii_digit());
        if !valid {
            return None;
        }
        end += 1;
    }
    None
}
