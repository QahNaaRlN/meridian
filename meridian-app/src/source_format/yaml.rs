//! Strict YAML subset adapter.
//!
//! `serde-saphyr` provides the YAML syntax layer. The compatibility parser
//! narrows its result to the observable subset of `scripts/lib/yaml.mjs`;
//! accepting more YAML would silently widen Meridian's input contract.

use std::fmt;

use serde_json::{Map, Number, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedYaml(pub String);

impl fmt::Display for UnsupportedYaml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UnsupportedYaml {}

#[derive(Clone)]
struct Line {
    indent: usize,
    text: String,
    number: usize,
}

/// Parse exactly the YAML subset implemented by the Node.js reference.
///
/// The strict line lint (tab indentation, a second document) and the
/// reference-subset reader run before the input reaches the library, as
/// `meridian-cli-rfc.md` decision D-A requires: otherwise the library's own
/// wording for the same rejection ("tabs disallowed within this context",
/// "unclosed bracket '['", …) would replace the reference's diagnostic. The
/// library then still rejects text the subset reader would accept although
/// it is not YAML at all.
pub fn parse(text: &str) -> Result<Value, UnsupportedYaml> {
    let mut lines = Vec::new();
    for (index, raw_with_cr) in text.split('\n').enumerate() {
        let raw = raw_with_cr.strip_suffix('\r').unwrap_or(raw_with_cr);
        if raw.starts_with('\t') {
            return Err(UnsupportedYaml(format!(
                "tab indentation at line {}",
                index + 1
            )));
        }
        if index > 0 && raw.trim_end() == "---" && raw.starts_with("---") {
            return Err(UnsupportedYaml(format!(
                "multi-document stream at line {}",
                index + 1
            )));
        }
        let stripped = strip_comment(raw);
        if stripped.trim().is_empty() {
            continue;
        }
        lines.push(Line {
            indent: stripped.bytes().take_while(|byte| *byte == b' ').count(),
            text: stripped.trim().to_owned(),
            number: index + 1,
        });
    }
    let value = match lines.first() {
        None => Value::Object(Map::new()),
        Some(first) => {
            let first_indent = first.indent;
            Parser { lines, position: 0 }.parse_node(first_indent)?
        }
    };
    if !text.trim().is_empty() {
        serde_saphyr::from_str::<Value>(text)
            .map_err(|error| UnsupportedYaml(format!("malformed YAML: {error}")))?;
    }
    Ok(value)
}

fn strip_comment(line: &str) -> &str {
    let mut single = false;
    let mut double = false;
    let mut previous_whitespace = false;
    for (index, character) in line.char_indices() {
        match character {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '#' if !single && !double && (index == 0 || previous_whitespace) => {
                return &line[..index]
            }
            _ => {}
        }
        previous_whitespace = character.is_whitespace();
    }
    line
}

fn block_scalar_style(value: &str) -> Result<Option<char>, UnsupportedYaml> {
    match value {
        "|" | "|-" => Ok(Some('|')),
        ">" | ">-" => Ok(Some('>')),
        "|+" | ">+" => Err(UnsupportedYaml(format!(
            "block scalar with keep chomping (\"{value}\") is not implemented by this reader"
        ))),
        _ => Ok(None),
    }
}

fn parse_scalar(raw: &str) -> Result<Value, UnsupportedYaml> {
    let value = raw.trim();
    if value.is_empty() || value == "~" || value == "null" {
        return Ok(Value::Null);
    }
    if value.starts_with('{') || value.starts_with('[') {
        return FlowParser::new(value).parse();
    }
    if value.starts_with('&') || value.starts_with('*') {
        return Err(UnsupportedYaml("anchor or alias".to_owned()));
    }
    if value.starts_with('!') {
        return Err(UnsupportedYaml("explicit tag".to_owned()));
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return Ok(Value::String(value[1..value.len() - 1].replace("''", "'")));
    }
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        return Ok(Value::String(
            value[1..value.len() - 1].replace("\\\"", "\""),
        ));
    }
    match value {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    if is_decimal_integer(value) {
        if let Ok(integer) = value.parse::<i64>() {
            return Ok(Value::Number(Number::from(integer)));
        }
        if let Ok(unsigned) = value.parse::<u64>() {
            return Ok(Value::Number(Number::from(unsigned)));
        }
    }
    if is_decimal_float(value) {
        if let Ok(float) = value.parse::<f64>() {
            if let Some(number) = Number::from_f64(float) {
                return Ok(Value::Number(number));
            }
        }
    }
    Ok(Value::String(value.to_owned()))
}

fn is_decimal_integer(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_decimal_float(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    let Some((whole, fraction)) = digits.split_once('.') else {
        return false;
    };
    !whole.is_empty()
        && !fraction.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

struct Parser {
    lines: Vec<Line>,
    position: usize,
}

impl Parser {
    fn parse_node(&mut self, indent: usize) -> Result<Value, UnsupportedYaml> {
        if self.position >= self.lines.len() || self.lines[self.position].indent < indent {
            return Ok(Value::Null);
        }
        if is_sequence_line(&self.lines[self.position].text) {
            self.parse_sequence(indent)
        } else {
            self.parse_mapping(indent)
        }
    }

    fn parse_sequence(&mut self, indent: usize) -> Result<Value, UnsupportedYaml> {
        let mut values = Vec::new();
        while self.position < self.lines.len()
            && self.lines[self.position].indent == indent
            && is_sequence_line(&self.lines[self.position].text)
        {
            let rest = self.lines[self.position]
                .text
                .strip_prefix("- ")
                .unwrap_or("")
                .trim()
                .to_owned();
            if rest.is_empty() {
                self.position += 1;
                values.push(self.parse_node(indent + 2)?);
                continue;
            }
            if let Some((key, inline)) = split_mapping_entry(&rest) {
                let key = key.to_owned();
                let inline = inline.to_owned();
                let item_indent = indent + 2;
                let mut object = Map::new();
                self.position += 1;
                let value = if let Some(style) = block_scalar_style(&inline)? {
                    self.read_block_scalar(item_indent, style)
                } else if inline.is_empty() {
                    if self.position < self.lines.len()
                        && self.lines[self.position].indent > item_indent
                    {
                        self.parse_node(self.lines[self.position].indent)?
                    } else {
                        Value::Null
                    }
                } else {
                    parse_scalar(&inline)?
                };
                object.insert(key, value);
                let more = self.parse_node(item_indent)?;
                if let Value::Object(more) = more {
                    object.extend(more);
                }
                values.push(Value::Object(object));
            } else {
                self.position += 1;
                values.push(parse_scalar(&rest)?);
            }
        }
        Ok(Value::Array(values))
    }

    fn parse_mapping(&mut self, indent: usize) -> Result<Value, UnsupportedYaml> {
        let mut object = Map::new();
        while self.position < self.lines.len() && self.lines[self.position].indent == indent {
            let line = self.lines[self.position].clone();
            if line.text.starts_with("- ") {
                break;
            }
            let Some((key, inline)) = split_mapping_entry(&line.text) else {
                return Err(UnsupportedYaml(format!(
                    "unrecognized construct at line {}: {}",
                    line.number, line.text
                )));
            };
            let key = key.to_owned();
            let inline = inline.to_owned();
            self.position += 1;
            let value = if let Some(style) = block_scalar_style(&inline)? {
                self.read_block_scalar(indent, style)
            } else if inline.is_empty() {
                if self.position < self.lines.len() && self.lines[self.position].indent > indent {
                    self.parse_node(self.lines[self.position].indent)?
                } else if self.position < self.lines.len()
                    && self.lines[self.position].indent == indent
                    && is_sequence_line(&self.lines[self.position].text)
                {
                    self.parse_node(indent)?
                } else {
                    Value::Null
                }
            } else {
                parse_scalar(&inline)?
            };
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }

    fn read_block_scalar(&mut self, parent_indent: usize, style: char) -> Value {
        let mut parts = Vec::new();
        while self.position < self.lines.len() && self.lines[self.position].indent > parent_indent {
            parts.push(self.lines[self.position].text.clone());
            self.position += 1;
        }
        Value::String(parts.join(if style == '|' { "\n" } else { " " }))
    }
}

fn is_sequence_line(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

fn split_mapping_entry(text: &str) -> Option<(&str, &str)> {
    let (key, value) = text.split_once(':')?;
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_$.-".contains(&byte))
        || (!value.is_empty() && !value.starts_with(char::is_whitespace))
    {
        return None;
    }
    Some((key, value.trim()))
}

struct FlowParser<'a> {
    source: &'a str,
    position: usize,
}

impl<'a> FlowParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
        }
    }

    fn parse(mut self) -> Result<Value, UnsupportedYaml> {
        let value = self.value()?;
        self.whitespace();
        if self.position != self.source.len() {
            return Err(UnsupportedYaml(
                "trailing content after flow collection".to_owned(),
            ));
        }
        Ok(value)
    }

    fn value(&mut self) -> Result<Value, UnsupportedYaml> {
        self.whitespace();
        match self.peek() {
            Some(b'[') => self.sequence(),
            Some(b'{') => self.mapping(),
            _ => self.token(false),
        }
    }

    fn sequence(&mut self) -> Result<Value, UnsupportedYaml> {
        self.position += 1;
        let mut values = Vec::new();
        self.whitespace();
        if self.consume(b']') {
            return Ok(Value::Array(values));
        }
        loop {
            values.push(self.value()?);
            self.whitespace();
            if self.consume(b',') {
                continue;
            }
            if self.consume(b']') {
                return Ok(Value::Array(values));
            }
            return Err(UnsupportedYaml("malformed flow sequence".to_owned()));
        }
    }

    fn mapping(&mut self) -> Result<Value, UnsupportedYaml> {
        self.position += 1;
        let mut object = Map::new();
        self.whitespace();
        if self.consume(b'}') {
            return Ok(Value::Object(object));
        }
        loop {
            let key = self.token_string(true)?;
            self.whitespace();
            if !self.consume(b':') {
                return Err(UnsupportedYaml("malformed flow mapping".to_owned()));
            }
            object.insert(key, self.value()?);
            self.whitespace();
            if self.consume(b',') {
                continue;
            }
            if self.consume(b'}') {
                return Ok(Value::Object(object));
            }
            return Err(UnsupportedYaml("malformed flow mapping".to_owned()));
        }
    }

    fn token(&mut self, key: bool) -> Result<Value, UnsupportedYaml> {
        if key {
            Ok(Value::String(self.token_string(true)?))
        } else {
            parse_scalar(&self.token_string(false)?)
        }
    }

    fn token_string(&mut self, key: bool) -> Result<String, UnsupportedYaml> {
        self.whitespace();
        if matches!(self.peek(), Some(b'\'') | Some(b'"')) {
            let quote = self.peek().expect("checked quote");
            self.position += 1;
            let start = self.position;
            while self.peek().is_some() && self.peek() != Some(quote) {
                self.position += 1;
            }
            if self.peek() != Some(quote) {
                return Err(UnsupportedYaml(
                    "unterminated quoted string in flow collection".to_owned(),
                ));
            }
            let result = self.source[start..self.position].to_owned();
            self.position += 1;
            return Ok(result);
        }
        let start = self.position;
        while let Some(byte) = self.peek() {
            let stop = byte == b',' || byte == b']' || byte == b'}' || (key && byte == b':');
            if stop {
                break;
            }
            self.position += 1;
        }
        Ok(self.source[start..self.position].trim().to_owned())
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(byte) if byte.is_ascii_whitespace()) {
            self.position += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }
    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, UnsupportedYaml};
    use serde_json::json;

    #[test]
    fn parses_block_and_flow_subset() {
        let value = parse("name: Meridian\nitems: [one, 2, {ok: true}]\n").unwrap();
        assert_eq!(
            value,
            json!({"name": "Meridian", "items": ["one", 2, {"ok": true}]})
        );
    }

    #[test]
    fn rejects_extensions() {
        for source in [
            "value: &anchor x\n",
            "value: !tag x\n",
            "value: x\n---\nvalue: y\n",
            "value: |+\n  x\n",
        ] {
            assert!(
                parse(source).is_err(),
                "source unexpectedly accepted: {source}"
            );
        }
    }

    /// Regression (`rust-business-contract-qualification`): the strict
    /// line lint ran after the library, so these two rejections carried the
    /// library's wording instead of the reference's
    /// (`scripts/lib/yaml.mjs`, `yamlParse`).
    #[test]
    fn the_strict_line_lint_names_the_rejection_before_the_library_does() {
        assert_eq!(
            parse("root:\n\tchild: value\n"),
            Err(UnsupportedYaml("tab indentation at line 2".to_owned()))
        );
        assert_eq!(
            parse("value: x\n---\nvalue: y\n"),
            Err(UnsupportedYaml(
                "multi-document stream at line 2".to_owned()
            ))
        );
        assert_eq!(
            parse("a: [1, 2\n"),
            Err(UnsupportedYaml("malformed flow sequence".to_owned()))
        );
        assert_eq!(
            parse("a: {b: 1\n"),
            Err(UnsupportedYaml("malformed flow mapping".to_owned()))
        );
    }

    /// Text the reference's subset reader accepts but that is not YAML is
    /// still rejected by the library after the reader has run (the reference
    /// would silently drop `c: d` / `b: 1`).
    #[test]
    fn text_that_is_not_yaml_is_still_rejected_by_the_library() {
        for source in ["a: \"unterminated\n", "a:\n  - b\n c: d\n", "- a\nb: 1\n"] {
            let error = parse(source).expect_err(source);
            assert!(error.0.starts_with("malformed YAML: "), "{source}: {error}");
        }
    }

    #[test]
    fn implicit_extensions_remain_strings() {
        let value = parse("hex: 0x10\nyes: yes\ndate: 2026-09-19\n").unwrap();
        assert_eq!(
            value,
            json!({"hex": "0x10", "yes": "yes", "date": "2026-09-19"})
        );
    }
}
