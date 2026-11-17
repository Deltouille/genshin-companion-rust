//! What each crate's tests check of its texts ([`check`]): that the
//! translations and the code agree, so a missing or misspelt text fails a
//! test rather than showing its key to a player.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};

use super::{FluentArgs, Language, Texts};

/// A text's variables, by its key.
type Messages = BTreeMap<String, BTreeSet<String>>;

/// A `t!` in the code: where, the key, and the variables it passes.
#[derive(Debug)]
struct Use {
    at: String,
    key: String,
    args: BTreeSet<String>,
}

/// Panics with every problem found:
///
/// - a text in English missing in another language, or the other way round;
/// - a text whose variables differ between languages;
/// - a `t!` in `crate_dir/src` naming a text English lacks, or passing other
///   variables than it uses;
/// - a text in English no `t!` names.
pub fn check(texts: &Texts, crate_dir: &Path) {
    let mut problems = Vec::new();
    let by_language: BTreeMap<Language, Messages> = Language::ALL
        .into_iter()
        .map(|language| (language, messages(texts, language, &mut problems)))
        .collect();
    let english = &by_language[&Language::English];
    for (language, messages) in &by_language {
        let code = language.code();
        for (key, variables) in english {
            match messages.get(key) {
                None => problems.push(format!("{code}: `{key}` is missing")),
                Some(theirs) if theirs != variables => problems.push(format!(
                    "{code}: `{key}` uses {theirs:?}, English {variables:?}"
                )),
                Some(_) => {}
            }
        }
        for key in messages.keys().filter(|key| !english.contains_key(*key)) {
            problems.push(format!("{code}: `{key}` isn't in English"));
        }
    }

    // Each text, formatted with every variable at 1: a function or a
    // reference that doesn't resolve shows here rather than to a player.
    for (language, messages) in &by_language {
        let bundle = &texts.bundles[language.index()];
        for (key, variables) in messages {
            let Some(pattern) = bundle.get_message(key).and_then(|m| m.value()) else {
                continue;
            };
            let mut args = FluentArgs::new();
            for variable in variables {
                args.set(variable.as_str(), 1);
            }
            let mut errors = Vec::new();
            bundle.format_pattern(pattern, Some(&args), &mut errors);
            if !errors.is_empty() {
                problems.push(format!(
                    "{}: `{key}` doesn't format: {errors:?}",
                    language.code()
                ));
            }
        }
    }

    let uses = uses(&crate_dir.join("src"), &mut problems);
    for found in &uses {
        match english.get(&found.key) {
            None => problems.push(format!("{}: no text `{}`", found.at, found.key)),
            Some(variables) if *variables != found.args => problems.push(format!(
                "{}: `{}` is given {:?}, uses {variables:?}",
                found.at, found.key, found.args
            )),
            Some(_) => {}
        }
    }
    let used: BTreeSet<&str> = uses.iter().map(|found| found.key.as_str()).collect();
    for key in english.keys().filter(|key| !used.contains(key.as_str())) {
        problems.push(format!("`{key}` is never used"));
    }

    assert!(problems.is_empty(), "texts:\n{}", problems.join("\n"));
}

/// Every text of one language and its variables.
fn messages(texts: &Texts, language: Language, problems: &mut Vec<String>) -> Messages {
    let mut messages = Messages::new();
    let files = (texts.sources.iter())
        .filter(|(candidate, _)| *candidate == language)
        .flat_map(|(_, files)| files.iter());
    for (name, text) in files {
        let resource = match fluent_syntax::parser::parse(*text) {
            Ok(resource) => resource,
            Err((_, errors)) => {
                problems.push(format!("{}/{name}.ftl: {errors:?}", language.code()));
                continue;
            }
        };
        for entry in resource.body {
            if let Entry::Message(message) = entry {
                let key = message.id.name.to_owned();
                let Some(value) = message.value else {
                    problems.push(format!("{}: `{key}` has no text", language.code()));
                    continue;
                };
                let mut variables = BTreeSet::new();
                pattern_variables(&value, &mut variables);
                if language == Language::French {
                    french_typography(&key, &value, problems);
                }
                messages.insert(key, variables);
            }
        }
    }
    messages
}

/// French typography, which a translator can't see in an editor: a no-break
/// space (U+00A0, or the narrow U+202F) before ? ! : ; and inside « »,
/// where a plain space would let the sign start a line, and the curly
/// apostrophe.
fn french_typography(key: &str, pattern: &Pattern<&str>, problems: &mut Vec<String>) {
    const NO_BREAK: [char; 2] = ['\u{a0}', '\u{202f}'];
    let mut previous: Option<char> = None;
    let mut texts = Vec::new();
    flatten(pattern, &mut texts);
    for piece in texts {
        let Some(text) = piece else {
            // A placeable: something stands before what follows.
            previous = Some('x');
            continue;
        };
        let chars: Vec<char> = text.chars().collect();
        for (ix, &c) in chars.iter().enumerate() {
            let before = if ix == 0 {
                previous
            } else {
                Some(chars[ix - 1])
            };
            let after = chars.get(ix + 1).copied();
            let wants_space_before = match c {
                '?' | '!' | ';' | '»' => true,
                // Not in "https://" nor between digits ("12:20").
                ':' => after != Some('/') && !before.is_some_and(|b| b.is_ascii_digit()),
                _ => false,
            };
            if wants_space_before && let Some(before) = before {
                if before == ' ' {
                    problems.push(format!(
                        "fr: `{key}`: a plain space before “{c}”: use a no-break space"
                    ));
                } else if !NO_BREAK.contains(&before) && !matches!(before, '?' | '!' | '(' | '\n') {
                    problems.push(format!("fr: `{key}`: no space before “{c}”"));
                }
            }
            if c == '«' && after.is_some_and(|after| !NO_BREAK.contains(&after)) {
                problems.push(format!("fr: `{key}`: “«” wants a no-break space after it"));
            }
            if c == '\'' {
                problems.push(format!("fr: `{key}`: a straight apostrophe: use ’"));
            }
        }
        if let Some(last) = chars.last() {
            previous = Some(*last);
        }
    }
}

/// A pattern's pieces in order: its text, and `None` for each placeable
/// that isn't text itself. A select's variants are each walked as if alone,
/// after what comes before the select.
fn flatten<'p>(pattern: &'p Pattern<&'p str>, out: &mut Vec<Option<&'p str>>) {
    for element in &pattern.elements {
        match element {
            PatternElement::TextElement { value } => out.push(Some(value)),
            PatternElement::Placeable {
                expression: Expression::Inline(InlineExpression::StringLiteral { value }),
            } => out.push(Some(value)),
            PatternElement::Placeable {
                expression: Expression::Select { variants, .. },
            } => {
                for variant in variants {
                    flatten(&variant.value, out);
                }
            }
            PatternElement::Placeable { .. } => out.push(None),
        }
    }
}

fn pattern_variables(pattern: &Pattern<&str>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            expression_variables(expression, out);
        }
    }
}

fn expression_variables(expression: &Expression<&str>, out: &mut BTreeSet<String>) {
    match expression {
        Expression::Select { selector, variants } => {
            inline_variables(selector, out);
            for variant in variants {
                pattern_variables(&variant.value, out);
            }
        }
        Expression::Inline(inline) => inline_variables(inline, out),
    }
}

fn inline_variables(inline: &InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match inline {
        InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_owned());
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            for argument in &arguments.positional {
                inline_variables(argument, out);
            }
            for argument in &arguments.named {
                inline_variables(&argument.value, out);
            }
        }
        InlineExpression::Placeable { expression } => expression_variables(expression, out),
        // A term's variables are its own; a message reference gets the
        // caller's, which are already counted where they are used.
        InlineExpression::StringLiteral { .. }
        | InlineExpression::NumberLiteral { .. }
        | InlineExpression::MessageReference { .. }
        | InlineExpression::TermReference { .. } => {}
    }
}

/// Every `t!` in the Rust files under `dir`.
fn uses(dir: &Path, problems: &mut Vec<String>) -> Vec<Use> {
    let mut files = Vec::new();
    rust_files(dir, &mut files);
    files.sort();
    let mut uses = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let code = blanked(&text);
        let bytes = code.as_bytes();
        let mut from = 0;
        while let Some(found) = code[from..].find("t!(") {
            let start = from + found;
            from = start + 3;
            let word_before = start
                .checked_sub(1)
                .is_some_and(|ix| bytes[ix].is_ascii_alphanumeric() || bytes[ix] == b'_');
            if word_before {
                continue;
            }
            let line = code[..start].matches('\n').count() + 1;
            let at = format!("{}:{line}", file.display());
            match parse_use(&code, &text, from) {
                Some((key, args)) => uses.push(Use { at, key, args }),
                None => problems.push(format!("{at}: a `t!` whose key isn't a literal")),
            }
        }
    }
    uses
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The code with its comments and the insides of its string and character
/// literals blanked, every byte at its place (newlines kept, so lines still
/// count): a `t!` in a doc comment or a string is not a use, and a comma in a
/// string doesn't end an argument.
fn blanked(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let blank = |out: &mut String, c: char| {
        if c == '\n' {
            out.push('\n');
        } else {
            out.extend(std::iter::repeat_n(' ', c.len_utf8()));
        }
    };
    let mut chars = code.chars().peekable();
    // The last character kept, to tell `r"` and `b'` from a name ending in r or b.
    let mut previous = ' ';
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                blank(&mut out, c);
                while let Some(c) = chars.next_if(|c| *c != '\n') {
                    blank(&mut out, c);
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                blank(&mut out, c);
                let mut last = ' ';
                for c in chars.by_ref() {
                    blank(&mut out, c);
                    if last == '*' && c == '/' {
                        break;
                    }
                    last = c;
                }
            }
            'r' if !previous.is_ascii_alphanumeric()
                && previous != '_'
                && matches!(chars.peek(), Some('"' | '#')) =>
            {
                // A raw string: r"…" or r#"…"#.
                out.push(c);
                let mut hashes = 0;
                while let Some(c) = chars.next_if_eq(&'#') {
                    out.push(c);
                    hashes += 1;
                }
                if chars.next_if_eq(&'"').is_none() {
                    previous = '#';
                    continue;
                }
                out.push('"');
                let mut closing = None;
                while let Some(c) = chars.next() {
                    if c == '"' {
                        let mut seen = 0;
                        while seen < hashes && chars.next_if_eq(&'#').is_some() {
                            seen += 1;
                        }
                        if seen == hashes {
                            closing = Some(seen);
                            break;
                        }
                        blank(&mut out, '"');
                        out.extend(std::iter::repeat_n(' ', seen));
                        continue;
                    }
                    blank(&mut out, c);
                }
                if let Some(seen) = closing {
                    out.push('"');
                    out.extend(std::iter::repeat_n('#', seen));
                }
                previous = '"';
                continue;
            }
            '"' => {
                out.push(c);
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            blank(&mut out, c);
                            if let Some(c) = chars.next() {
                                blank(&mut out, c);
                            }
                        }
                        '"' => {
                            out.push(c);
                            break;
                        }
                        _ => blank(&mut out, c),
                    }
                }
            }
            '\'' => {
                // A character literal ('x', '\n', '\u{2212}'), else a lifetime.
                let mut ahead = chars.clone();
                let literal = match ahead.next() {
                    Some('\\') => true,
                    Some(_) => ahead.next() == Some('\''),
                    None => false,
                };
                out.push(c);
                if literal {
                    while let Some(c) = chars.next() {
                        match c {
                            '\\' => {
                                blank(&mut out, c);
                                if let Some(c) = chars.next() {
                                    blank(&mut out, c);
                                }
                            }
                            '\'' => {
                                out.push(c);
                                break;
                            }
                            _ => blank(&mut out, c),
                        }
                    }
                }
            }
            _ => out.push(c),
        }
        previous = c;
    }
    out
}

/// The key and variable names of a `t!` whose `(` ends at `from`: its
/// shape read in the blanked code, its key in the code itself.
fn parse_use(blanked: &str, code: &str, from: usize) -> Option<(String, BTreeSet<String>)> {
    let skip_space = |at: usize| at + blanked[at..].len() - blanked[at..].trim_start().len();
    let open = skip_space(from);
    if !blanked[open..].starts_with('"') {
        return None;
    }
    let close = open + 1 + blanked[open + 1..].find('"')?;
    let key = code[open + 1..close].to_owned();
    let mut rest = &blanked[close + 1..];
    let mut args = BTreeSet::new();
    loop {
        rest = rest.trim_start();
        if rest.starts_with(')') {
            return Some((key, args));
        }
        rest = rest.strip_prefix(',')?.trim_start();
        if rest.starts_with(')') {
            return Some((key, args));
        }
        let name_end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
        if name_end == 0 {
            return None;
        }
        args.insert(rest[..name_end].to_owned());
        rest = rest[name_end..].trim_start().strip_prefix('=')?;
        rest = skip_expression(rest)?;
    }
}

/// What follows an argument's expression (in blanked code): the `,` or `)`
/// that ends it.
fn skip_expression(rest: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (ix, c) in rest.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ',' | ')' if depth == 0 => return Some(&rest[ix..]),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(code: &str) -> Option<(String, Vec<String>)> {
        let code = format!("t!({code}");
        parse_use(&blanked(&code), &code, 3).map(|(key, args)| (key, args.into_iter().collect()))
    }

    #[test]
    fn a_use_names_its_key_and_variables() {
        assert_eq!(parsed(r#""plain")"#), Some(("plain".to_owned(), vec![])));
        assert_eq!(
            parsed(
                "\n    \"wishes\",\n    count = rows.len(),\n    name = &format!(\"{a}, {b})\"),\n)"
            ),
            Some((
                "wishes".to_owned(),
                vec!["count".to_owned(), "name".to_owned()]
            ))
        );
        assert_eq!(parsed("key)"), None);
    }

    #[test]
    fn french_typography_is_checked() {
        let source = "plain = Restaurer ?\nnone = Restaurer?\nright = Restaurer\u{a0}? «\u{a0}x\u{a0}» à 12:20, https://x\n\
                      quotes = « x »\nafter = { $what }: x\nselect = { $n ->\n    [one] un\n   *[other] { $n }\n}\u{a0}: x\n\
                      apostrophe = l'app\n";
        let resource = fluent_syntax::parser::parse(source).unwrap();
        let mut problems = Vec::new();
        for entry in &resource.body {
            if let Entry::Message(message) = entry {
                french_typography(
                    message.id.name,
                    message.value.as_ref().unwrap(),
                    &mut problems,
                );
            }
        }
        let flagged: Vec<&str> = problems
            .iter()
            .map(|p| p.split('`').nth(1).unwrap())
            .collect();
        assert_eq!(
            flagged,
            ["plain", "none", "quotes", "quotes", "after", "apostrophe"],
            "{problems:#?}"
        );
    }

    #[test]
    fn comments_and_literals_are_not_uses() {
        let code = "// t!(\"commented\")\nlet a = \"t!(x) // kept\"; /* t!(\"block\") */ \
                    let q = '\"'; let r = r#\"t!(\"raw\")\"#; t!(\"real\")";
        let blanked = blanked(code);
        assert_eq!(blanked.len(), code.len());
        assert_eq!(blanked.matches("t!(").count(), 1, "{blanked}");
        assert!(blanked.ends_with("t!(\"    \")"), "{blanked}");
        assert_eq!(blanked.matches('\n').count(), 1);
    }
}
