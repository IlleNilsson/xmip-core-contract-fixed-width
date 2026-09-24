#![forbid(unsafe_code)]

//! The fixed-width content contract — a technology of `xmip-core-contract`.
//!
//! Two claims, decided 2026-09-07: **well-formedness is a given** — the Stream
//! is text — and **conformance is a given once a contract is named**: a Receive
//! or Send Location that refers to this contract with a layout bound has every
//! record held to it.
//!
//! The layout language is the COBOL copybook, because that is what fixed-width
//! files come with: a mainframe hands over a `.cpy` beside the data, and the
//! flat-file schemas of the products Xmip replaces were hand-transcriptions of
//! it. [`copybook`] documents the subset read. Records are lines when the text
//! has line breaks, and consecutive record-length slices when it does not.

pub mod copybook;

use contract::{
    Contract, ContractDescriptor, ContractError, ContractFactory, ContractId, ValidationIssue,
    ValidationResult,
};
use copybook::{Field, Layout};
use stream::Stream;

/// The fixed-width contract, bare or bound to a layout.
pub struct FixedWidth {
    descriptor: ContractDescriptor,
    layout: Option<Layout>,
}

impl FixedWidth {
    /// Text only.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: descriptor("fixed-width"),
            layout: None,
        }
    }

    /// Text laid out by the copybook `text`.
    ///
    /// # Errors
    /// The copybook must be within the subset [`copybook`] documents.
    pub fn with_copybook(text: &str) -> Result<Self, ContractError> {
        let layout = Layout::parse(text)?;
        let name = layout.record_name().to_string();
        Ok(Self {
            descriptor: descriptor(&format!("fixed-width:{name}")),
            layout: Some(layout),
        })
    }

    /// Whether a layout is bound.
    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.layout.is_some()
    }
}

impl Default for FixedWidth {
    fn default() -> Self {
        Self::new()
    }
}

fn descriptor(id: &str) -> ContractDescriptor {
    ContractDescriptor {
        id: ContractId(id.to_string()),
        version: "1".to_string(),
        representation: "text/plain".to_string(),
    }
}

impl Contract for FixedWidth {
    fn descriptor(&self) -> &ContractDescriptor {
        &self.descriptor
    }

    fn identify(&self, stream: &Stream) -> Result<bool, ContractError> {
        // Text is the claim; a layout narrows nothing here, so a wrong-length
        // file is reported by validate rather than silently unclaimed.
        Ok(std::str::from_utf8(stream.bytes()).is_ok())
    }

    fn validate(&self, stream: &Stream) -> Result<ValidationResult, ContractError> {
        let text = match std::str::from_utf8(stream.bytes()) {
            Ok(text) => text,
            Err(error) => {
                return Ok(ValidationResult::of(vec![ValidationIssue::at(
                    "malformed",
                    &format!("not UTF-8 text: {error}"),
                    &format!("byte {}", error.valid_up_to()),
                )]));
            }
        };
        let Some(layout) = &self.layout else {
            return Ok(ValidationResult::of(Vec::new()));
        };
        let mut issues = Vec::new();
        for (ordinal, record) in records(text, layout.record_length()).enumerate() {
            check_record(layout, record, ordinal + 1, &mut issues);
        }
        Ok(ValidationResult::of(issues))
    }
}

/// Records are lines when the text has any, else slices of the record length.
fn records(text: &str, length: usize) -> Box<dyn Iterator<Item = &str> + '_> {
    if text.contains('\n') {
        Box::new(
            text.lines()
                .map(|l| l.strip_suffix('\r').unwrap_or(l))
                .filter(|l| !l.is_empty()),
        )
    } else {
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let starts: Vec<usize> = chars
            .iter()
            .step_by(length.max(1))
            .map(|(i, _)| *i)
            .collect();
        Box::new(starts.into_iter().map(move |start| {
            let end = text[start..]
                .char_indices()
                .nth(length)
                .map_or(text.len(), |(i, _)| start + i);
            &text[start..end]
        }))
    }
}

fn check_record(layout: &Layout, record: &str, ordinal: usize, out: &mut Vec<ValidationIssue>) {
    let chars: Vec<char> = record.chars().collect();
    let at = format!("record {ordinal}");
    if chars.len() != layout.record_length() {
        let message = format!(
            "is {} characters, the layout is {}",
            chars.len(),
            layout.record_length()
        );
        out.push(ValidationIssue::at("length", &message, &at));
        return;
    }
    for field in layout.fields() {
        let value: String = chars[field.offset..field.offset + field.length]
            .iter()
            .collect();
        if let Some(message) = departure(field, &value) {
            out.push(ValidationIssue::at(
                "value",
                &message,
                &format!("{at} / {}", field.name),
            ));
        }
    }
}

/// Why a value does not fit its field, or `None` when it does.
fn departure(field: &Field, value: &str) -> Option<String> {
    if !field.numeric {
        return None;
    }
    let trimmed = value.trim();
    if trimmed.is_empty() {
        // An unused occurrence is blank on every mainframe file ever handed over.
        return None;
    }
    let digits = trimmed
        .trim_start_matches(['+', '-'])
        .trim_end_matches(['+', '-']);
    let signs = trimmed.len() - digits.len();
    if signs > 1 || (signs == 1 && !field.signed) {
        return Some(format!("{trimmed:?} carries a sign the picture does not"));
    }
    if !digits
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return Some(format!("{trimmed:?} is not numeric"));
    }
    None
}

/// Loads the contract a Location names: an empty reference is the bare
/// contract, anything else is the path of a copybook file.
pub struct FixedWidthFactory;

impl ContractFactory for FixedWidthFactory {
    fn technology(&self) -> &'static str {
        "fixed-width"
    }

    fn load(&self, reference: &str) -> Result<Box<dyn Contract>, ContractError> {
        if reference.trim().is_empty() {
            return Ok(Box::new(FixedWidth::new()));
        }
        let text = std::fs::read_to_string(reference).map_err(|error| ContractError {
            message: format!("cannot read copybook {reference}: {error}"),
        })?;
        Ok(Box::new(FixedWidth::with_copybook(&text)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract::fixture::stream;
    use xcore::StreamId;

    const ORDER: &str = "
       01  ORDER-RECORD.
           05  ORDER-ID        PIC X(6).
           05  CUSTOMER        PIC X(10).
           05  LINE-COUNT      PIC 9(2).
           05  ORDER-LINE      OCCURS 2 TIMES.
               10  SKU         PIC X(4).
               10  QTY         PIC S9(3).
           05  TOTAL           PIC 9(5)V99.
    ";

    #[test]
    fn bare_contract_holds_text_only() {
        assert!(
            FixedWidth::new()
                .validate(&stream("anything"))
                .expect("validates")
                .valid
        );
        let broken = FixedWidth::new()
            .validate(&stream_bytes(&[0xff]))
            .expect("validates");
        assert_eq!(broken.issues[0].code, "malformed");
    }

    fn stream_bytes(bytes: &[u8]) -> Stream {
        Stream::new(StreamId::new(1), bytes.to_vec(), None)
    }

    #[test]
    fn bound_contract_holds_conforming_records() {
        let bound = FixedWidth::with_copybook(ORDER).expect("copybook");
        assert_eq!(bound.descriptor().id.0, "fixed-width:ORDER-RECORD");
        // ORDER-ID CUSTOMER   LC SKU QTY SKU QTY TOTAL — a blank second line is
        // an unused occurrence, which every mainframe file has.
        let text =
            "A00001ACME      02X001+02X0020100001500\nA00002BOLT      01X003-01       0000099\n";
        let held = bound.validate(&stream(text)).expect("validates");
        assert!(held.valid, "issues: {:?}", held.issues);
    }

    #[test]
    fn bound_contract_names_the_record_and_field_that_departs() {
        let bound = FixedWidth::with_copybook(ORDER).expect("copybook");
        let text = "A00001ACME      02X001+02X0020100001500\nshort\n\
                    A00003ACME      xxX001  1X00201000015AB\n";
        let held = bound.validate(&stream(text)).expect("validates");
        let seen: Vec<(&str, &str)> = held
            .issues
            .iter()
            .map(|i| (i.code.as_str(), i.path.as_deref().unwrap_or("")))
            .collect();
        assert_eq!(
            seen,
            [
                ("length", "record 2"),
                ("value", "record 3 / LINE-COUNT"),
                ("value", "record 3 / TOTAL"),
            ],
            "{:?}",
            held.issues
        );
    }

    #[test]
    fn records_without_line_breaks_are_sliced_to_the_layout() {
        let bound = FixedWidth::with_copybook(ORDER).expect("copybook");
        let one = "A00001ACME      02X001+02X0020100001500";
        let text = format!("{one}{one}");
        assert!(bound.validate(&stream(&text)).expect("validates").valid);
        let short = format!("{one}A0");
        let held = bound.validate(&stream(&short)).expect("validates");
        assert_eq!(held.issues[0].path.as_deref(), Some("record 2"));
    }

    #[test]
    fn the_factory_loads_bare_and_bound() {
        let factory = FixedWidthFactory;
        assert_eq!(factory.technology(), "fixed-width");
        assert_eq!(
            factory.load("").expect("bare").descriptor().id.0,
            "fixed-width"
        );
        let dir = std::env::temp_dir().join("xmip-fixed-width-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("order.cpy");
        std::fs::write(&file, ORDER).expect("write copybook");
        let bound = factory
            .load(file.to_str().expect("utf-8 path"))
            .expect("bound");
        assert_eq!(bound.descriptor().id.0, "fixed-width:ORDER-RECORD");
        assert!(
            factory
                .load(dir.join("missing.cpy").to_str().expect("path"))
                .is_err()
        );
    }
}
