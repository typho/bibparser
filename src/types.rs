use std::collections::HashMap;

/// One entry in a `.bib` file
#[derive(Debug, Clone)]
pub struct BibEntry {
    /// entry type, e.g. “article”
    pub kind: String,
    /// entry name, e.g. “DBLP:books/lib/Knuth97”
    pub id: String,
    /// map of fields, e.g. “author” mapped to “Donald Ervin Knuth”
    pub fields: HashMap<String, String>,
}

impl BibEntry {
    /// Generate a new, empty instance of BibEntry. Can also be called through the `Default` implementation.
    pub fn new() -> BibEntry {
        BibEntry {
            kind: String::new(),
            id: String::new(),
            fields: HashMap::new(),
        }
    }

    /// Removes Teχ's groups from a string. For example,
    /// given a string like “Written by {{Lukas} and {tajpulo}}”
    /// returns “Written by Lukas and tajpulo”
    pub fn degroup(src: &str) -> String {
        {
            let mut result = String::new();
            let mut level = 0;
            let mut escape = false;
            for chr in src.chars() {
                if chr == '{' && !escape {
                    level += 1;
                } else if chr == '}' && !escape {
                    level -= 1;
                } else if chr == '\\' {
                    if escape {
                        result.push(chr);
                    }
                    escape = !escape;
                } else {
                    if escape {
                        result.push('\\');
                    }
                    result.push(chr);
                    escape = false;
                }
            }
            if level == 0 {
                return result;
            }
        }
        src.to_string()
    }

    /// Reduce the whitespace according to free form semantics
    /// common in markup languages. Multiple whitespace sequences
    /// are merged. For example, “a message.  \nBest  regards”
    /// becomes “a message. Best regards”. Empty lines defining
    /// paragraphs are not supported.
    pub fn reduce_whitespace(src: &str) -> String {
        let mut result = String::new();
        let mut was_whitespace = false;
        for chr in src.chars() {
            if chr.is_whitespace() {
                if !was_whitespace {
                    result.push(chr);
                }
                was_whitespace = true;
            } else {
                result.push(chr);
            }
        }
        result
    }

    /// Given the name of a field, return its `data` the closest Unicode representation
    /// assuming Teχ semantics for the `data`. In particular …
    /// 
    /// * replace “---” and “--” by en-dash and em-dash respectively
    /// * replace the “LaTeχ” control sequence
    /// * replace escaped sequences with their semantic representation
    /// * replace “~” by a non-breaking space
    /// * remove groups and reduce whitespace
    /// 
    /// If you think, we miss something, please file a bug report.
    pub fn unicode_data(&self, field_name: &str) -> Option<String> {
        match self.fields.get(field_name) {
            Some(data) => {
                let replacements = [
                    ("---", "—"),
                    ("--", "–"),
                    ("\\LaTeX{}", "LaTeχ"),
                    ("{\\LaTeX}", "LaTeχ"),
                    ("\\LaTeX", "LaTeχ"),
                    ("\\\"", "\""),
                    ("\\&", "&"),
                    ("~", "\u{00A0}"),
                ];

                let mut result = data.clone();
                for (pattern, replacement) in replacements.iter() {
                    result = result.replace(pattern, replacement);
                }
                result = Self::degroup(&result);
                result = Self::reduce_whitespace(&result);
                Some(result)
            }
            None => None,
        }
    }
}

impl Default for BibEntry {
    fn default() -> Self {
        Self::new()
    }
}
