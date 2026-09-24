//! The COBOL copybook subset this contract binds, and the record layout it
//! becomes.
//!
//! Read: level numbers 01 to 49 and 77; group items and elementary items;
//! `PIC` / `PICTURE` with `X`, `A`, `9`, `S`, `V`, `Z`, `*`, `-`, `+`, `.`,
//! `,` and `(n)` repeats; `OCCURS n TIMES`; `REDEFINES`, which shares its
//! target's offset; `FILLER`; `VALUE` and `JUSTIFIED`, ignored; level 88
//! condition names and level 66 renames, ignored; `*` comment lines; the
//! six-column sequence area of fixed-format source. A statement ends at a
//! full stop.
//!
//! Refused when bound, by name: `USAGE` other than `DISPLAY` — `COMP`,
//! `COMP-3`, `BINARY`, `PACKED-DECIMAL` — because those fields are not text
//! and this contract's first claim is that the Stream is; `OCCURS DEPENDING
//! ON`, `SIGN SEPARATE`, and `COPY` inside a copybook.
//!
//! A field's name in an issue is its own name, with the occurrence when it
//! repeats: `ORDER-LINE(2).QTY`.

use sdk::contract::ContractError;

/// One elementary field in a record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub offset: usize,
    pub length: usize,
    pub numeric: bool,
    pub signed: bool,
}

/// A record layout read from a copybook.
pub struct Layout {
    record_name: String,
    record_length: usize,
    fields: Vec<Field>,
}

struct Item {
    level: u8,
    name: String,
    picture: Option<String>,
    occurs: usize,
    redefines: Option<String>,
}

impl Layout {
    /// Read a copybook.
    ///
    /// # Errors
    /// Outside the subset, or no record at all.
    pub fn parse(text: &str) -> Result<Self, ContractError> {
        let items = items(text)?;
        let Some(first) = items.first() else {
            return Err(refuse("no data item"));
        };
        let mut fields = Vec::new();
        let (end, _) = place(&items, 0, first.level, 0, "", &mut fields)?;
        Ok(Self {
            record_name: first.name.clone(),
            record_length: end,
            fields,
        })
    }

    #[must_use]
    pub fn record_name(&self) -> &str {
        &self.record_name
    }

    #[must_use]
    pub fn record_length(&self) -> usize {
        self.record_length
    }

    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
}

/// Lay out the items from `index` at `level`, starting at `offset`. Returns the
/// offset after them and the index of the first item not consumed.
fn place(
    items: &[Item],
    mut index: usize,
    level: u8,
    start: usize,
    prefix: &str,
    out: &mut Vec<Field>,
) -> Result<(usize, usize), ContractError> {
    let mut offset = start;
    let mut placed: Vec<(String, usize, usize)> = Vec::new(); // name, offset, length
    while index < items.len() && items[index].level == level {
        let item = &items[index];
        let name = if prefix.is_empty() {
            item.name.clone()
        } else {
            format!("{prefix}.{}", item.name)
        };
        let origin = match &item.redefines {
            Some(target) => placed
                .iter()
                .find(|(n, _, _)| n.rsplit('.').next() == Some(target.as_str()))
                .map(|(_, o, _)| *o)
                .ok_or_else(|| refuse(format!("{} REDEFINES unknown {target}", item.name)))?,
            None => offset,
        };
        let mut one = 0;
        let mut next = index + 1;
        for occurrence in 1..=item.occurs {
            let occurrence_name = if item.occurs > 1 {
                format!("{name}({occurrence})")
            } else {
                name.clone()
            };
            let at = origin + one * (occurrence - 1);
            // The record itself does not prefix its fields: an operator reads
            // `CUSTOMER`, not `ORDER-RECORD.CUSTOMER`.
            let child_prefix = if prefix.is_empty() && item.level == items[0].level {
                String::new()
            } else {
                occurrence_name.clone()
            };
            let (end, after) = if let Some(picture) = &item.picture {
                let (length, numeric, signed) = measure(picture)?;
                if item.name != "FILLER" {
                    out.push(Field {
                        name: occurrence_name,
                        offset: at,
                        length,
                        numeric,
                        signed,
                    });
                }
                (at + length, index + 1)
            } else {
                let child_level = items.get(index + 1).map(|i| i.level);
                if let Some(child) = child_level.filter(|child| *child > level) {
                    place(items, index + 1, child, at, &child_prefix, out)?
                } else {
                    (at, index + 1)
                }
            };
            one = end - at;
            next = after;
        }
        let length = one * item.occurs;
        placed.push((name, origin, length));
        if item.redefines.is_none() {
            offset = origin + length;
        } else {
            offset = offset.max(origin + length);
        }
        index = next;
    }
    Ok((offset, index))
}

/// A picture's storage length, and whether it is numeric and signed.
fn measure(picture: &str) -> Result<(usize, bool, bool), ContractError> {
    let mut length = 0;
    let mut numeric = true;
    let mut signed = false;
    let mut chars = picture.chars().peekable();
    while let Some(symbol) = chars.next() {
        let mut repeat = 1;
        if chars.peek() == Some(&'(') {
            chars.next();
            let digits: String = chars.by_ref().take_while(|c| *c != ')').collect();
            repeat = digits
                .trim()
                .parse()
                .map_err(|_| refuse(format!("picture {picture:?} has a bad repeat")))?;
        }
        match symbol.to_ascii_uppercase() {
            'X' | 'A' => {
                numeric = false;
                length += repeat;
            }
            '9' | 'Z' | '*' | '.' | ',' | '-' | '+' | 'B' | '0' | '/' => length += repeat,
            'S' => signed = true,
            'V' | 'P' => {}
            other => return Err(refuse(format!("picture symbol {other:?} is not supported"))),
        }
    }
    Ok((length, numeric, signed))
}

fn items(text: &str) -> Result<Vec<Item>, ContractError> {
    let mut source = String::new();
    for raw in text.lines() {
        let line = sequence_stripped(raw);
        if line.trim_start().starts_with('*') || line.trim().is_empty() {
            continue;
        }
        source.push_str(line);
        source.push(' ');
    }
    let mut items = Vec::new();
    for statement in source.split('.').map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(item) = item(statement)? {
            items.push(item);
        }
    }
    Ok(items)
}

/// Fixed-format source carries a six-column sequence area and an indicator
/// column; free-format does not. A line whose first six characters are digits
/// or spaces followed by a space or `*` is fixed-format.
fn sequence_stripped(line: &str) -> &str {
    let bytes = line.as_bytes();
    let fixed = bytes.len() > 6
        && bytes[..6].iter().all(|b| b.is_ascii_digit() || *b == b' ')
        && matches!(bytes[6], b' ' | b'*' | b'-')
        && bytes[..6].iter().any(u8::is_ascii_digit);
    if fixed { &line[6..] } else { line }
}

fn item(statement: &str) -> Result<Option<Item>, ContractError> {
    let words: Vec<&str> = statement.split_whitespace().collect();
    let [level, name, rest @ ..] = words.as_slice() else {
        return Err(refuse(format!("cannot read {statement:?}")));
    };
    let level: u8 = level
        .parse()
        .map_err(|_| refuse(format!("cannot read level in {statement:?}")))?;
    if level == 88 || level == 66 {
        return Ok(None);
    }
    let mut item = Item {
        level,
        name: name.to_ascii_uppercase(),
        picture: None,
        occurs: 1,
        redefines: None,
    };
    let mut clause = rest.iter().map(|w| w.to_ascii_uppercase()).peekable();
    while let Some(word) = clause.next() {
        match word.as_str() {
            "PIC" | "PICTURE" => {
                let mut picture = clause
                    .next()
                    .ok_or_else(|| refuse("PIC without a picture"))?;
                if picture == "IS" {
                    picture = clause
                        .next()
                        .ok_or_else(|| refuse("PIC IS without a picture"))?;
                }
                item.picture = Some(picture);
            }
            "OCCURS" => {
                let count = clause
                    .next()
                    .ok_or_else(|| refuse("OCCURS without a count"))?;
                item.occurs = count
                    .parse()
                    .map_err(|_| refuse(format!("OCCURS {count} is not a count")))?;
                if clause.peek().is_some_and(|w| w == "TO") {
                    return Err(refuse("OCCURS DEPENDING ON is not supported"));
                }
            }
            "DEPENDING" => return Err(refuse("OCCURS DEPENDING ON is not supported")),
            "REDEFINES" => {
                item.redefines = clause
                    .next()
                    .ok_or_else(|| refuse("REDEFINES without a name"))?
                    .into();
            }
            "COMP" | "COMP-1" | "COMP-2" | "COMP-3" | "COMP-4" | "COMP-5" | "COMPUTATIONAL"
            | "COMPUTATIONAL-3" | "BINARY" | "PACKED-DECIMAL" | "POINTER" | "INDEX" => {
                return Err(refuse(format!(
                    "{} in {}: only DISPLAY usage is text",
                    word, item.name
                )));
            }
            "SEPARATE" => return Err(refuse("SIGN SEPARATE is not supported")),
            "COPY" => return Err(refuse("COPY inside a copybook is not supported")),
            _ => {}
        }
    }
    Ok(Some(item))
}

fn refuse(reason: impl std::fmt::Display) -> ContractError {
    ContractError {
        message: format!("copybook refused: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_measures_its_storage() {
        assert_eq!(measure("X(10)").expect("ok"), (10, false, false));
        assert_eq!(measure("S9(5)V99").expect("ok"), (7, true, true));
        assert_eq!(measure("999").expect("ok"), (3, true, false));
        assert_eq!(measure("ZZ9.99").expect("ok"), (6, true, false));
        assert!(measure("Q(3)").is_err());
    }

    #[test]
    fn occurs_and_redefines_lay_out_as_cobol_does() {
        let text = "
000100 01 REC.
000200    05 KIND      PIC X.
000300    05 BODY      PIC X(8).
000400    05 BODY-NUM  REDEFINES BODY PIC 9(8).
000500    05 ITEM      OCCURS 3 TIMES.
000600       10 CODE   PIC X(2).
000700       10 AMT    PIC 9(3).
000800    05 FILLER    PIC X(2).
        ";
        let layout = Layout::parse(text).expect("parses");
        assert_eq!(layout.record_name(), "REC");
        assert_eq!(layout.record_length(), 1 + 8 + 15 + 2);
        let names: Vec<(&str, usize)> = layout
            .fields()
            .iter()
            .map(|f| (f.name.as_str(), f.offset))
            .collect();
        assert_eq!(
            names,
            [
                ("KIND", 0),
                ("BODY", 1),
                ("BODY-NUM", 1),
                ("ITEM(1).CODE", 9),
                ("ITEM(1).AMT", 11),
                ("ITEM(2).CODE", 14),
                ("ITEM(2).AMT", 16),
                ("ITEM(3).CODE", 19),
                ("ITEM(3).AMT", 21),
            ]
        );
    }

    #[test]
    fn binary_usage_is_refused_by_name() {
        let error = Layout::parse("01 R. 05 N PIC S9(4) COMP-3.")
            .err()
            .expect("refused");
        assert!(error.message.contains("COMP-3"), "{}", error.message);
    }

    #[test]
    fn comments_and_condition_names_are_skipped() {
        let text = "      * a comment\n 01 R.\n   05 F PIC X.\n   88 F-YES VALUE 'Y'.\n";
        let layout = Layout::parse(text).expect("parses");
        assert_eq!(layout.fields().len(), 1);
    }
}
