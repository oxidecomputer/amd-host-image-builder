/** Serializer that can do the following:
- Serialize to JSON5 with little noise
- If a field was skipped, note on stderr what was skipped
 */
use serde::{ser, Serialize};
use std::fmt::{Display, Formatter};
use std::io::Write;

use unic_ucd_ident::{is_id_continue, is_id_start};

// Return None if there was a parse error
fn property_key_parse_symbol(key: &str) -> Option<&str> {
    if key.is_empty() {
        return None;
    }
    let reserved_words = vec![
        "switch",
        "case",
        "break",
        "try",
        "catch",
        "class",
        "const",
        "var",
        "let",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "export",
        "extends",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "new",
        "return",
        "super",
        "this",
        "throw",
        "typeof",
        "void",
        "while",
        "with",
        "yield",
        "static",
        "enum",
        "await",
        "implements",
        "interface",
        "package",
        "protected",
        "private",
        "public",
        "null",
        "true",
        "false",
    ];
    if reserved_words.contains(&key) {
        return None;
    }
    let mut chars = key.chars();
    let mut first = true;
    loop {
        let ch = match chars.next() {
            Some(x) => x,
            None => return Some(key),
        };
        if !is_id_continue(ch)
            && !matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$')
        {
            // Check whether it's an Unicode escape sequence \uxxxx or \u{xxxxxxxx}
            if ch == '\\' {
                let ch = chars.next()?;
                if ch == 'u' {
                    let ch = chars.next()?;
                    if ch == '{' {
                        loop {
                            let ch = chars.next()?;
                            if ch == '}' {
                                break;
                            }
                            ch.is_ascii_hexdigit().then_some(())?;
                        }
                    } else {
                        ch.is_ascii_hexdigit().then_some(())?;
                        for _i in 1..4 {
                            let ch = chars.next()?;
                            ch.is_ascii_hexdigit().then_some(())?;
                        }
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else if first {
            first = false;
            if !is_id_start(ch)
                && !matches!(ch, 'a'..='z' | 'A'..='Z' | '_' | '$')
            {
                return None;
            }
        }
    }
}

fn property_key_needs_quoting(key: &str) -> bool {
    property_key_parse_symbol(key).is_none()
}

#[test]
fn test_property_key_needs_quoting() {
    assert!(property_key_needs_quoting(""));
    assert!(!property_key_needs_quoting("a"));
    assert!(property_key_needs_quoting("@"));
    assert!(!property_key_needs_quoting("aa"));
    assert!(property_key_needs_quoting("1a"));
    assert!(!property_key_needs_quoting("a1"));
    assert!(!property_key_needs_quoting("abc"));
    assert!(!property_key_needs_quoting("a1a"));
    assert!(property_key_needs_quoting("abc@"));
    assert!(property_key_needs_quoting("@"));
    assert!(property_key_needs_quoting("@abc"));
    assert!(property_key_needs_quoting("@a"));
    assert!(!property_key_needs_quoting("_validIdentifier"));
    assert!(property_key_needs_quoting("123abc"));
    assert!(property_key_needs_quoting("function"));
    assert!(!property_key_needs_quoting("über"));
    assert!(!property_key_needs_quoting("\\u{10000}start"));
    assert!(!property_key_needs_quoting(r"\u1000start"));
    //assert!(!property_key_needs_quoting("🚀Rocket"));
}

/// Writes V as a JSON string literal
fn serialize_string<W: Write>(writer: &mut W, v: &str) -> Result<(), Error> {
    write!(writer, "\"")?;
    if v.contains('"') {
        write!(
            writer,
            "{}",
            v.replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('"', "\\\"")
        )?;
    } else {
        write!(writer, "{}", v)?;
    }
    write!(writer, "\"")?;
    Ok(())
}

#[test]
fn test_serialize_string() {
    fn serialize1(v: &str) -> String {
        let mut buf = std::io::BufWriter::new(Vec::<u8>::new());
        serialize_string(&mut buf, v).unwrap();
        String::from_utf8(buf.into_inner().unwrap()).unwrap()
    }
    assert_eq!(serialize1(""), "\"\"");
    assert_eq!(serialize1("h"), "\"h\"");
    assert_eq!(serialize1("hello"), "\"hello\"");
    assert_eq!(serialize1("hello world"), "\"hello world\"");
    assert_eq!(serialize1("he said \"hello\""), "\"he said \\\"hello\\\"\"");
}

pub struct Json5Serializer<W: Write> {
    writer: W,
    line_number: u64,
    indent: usize,
    path: Vec<String>,
    current_indent: usize,
    /// JSON5 supports symbols as map keys.
    /// So we have SerializeMap set expect_symbol and any fn serialize_* reset expect_symbol.
    /// serialize_str is then actually writing either a symbol or a string literal depending on the need.
    expect_symbol: bool,
}

impl<W: Write> Json5Serializer<W> {
    fn new(writer: W) -> Self {
        Json5Serializer {
            writer,
            line_number: 1,
            path: Vec::<String>::new(),
            indent: 2,
            current_indent: 0,
            expect_symbol: false,
        }
    }

    fn increase_line_number(&mut self) {
        self.line_number += 1;
    }

    fn serialize_symbol(&mut self, v: &str) -> Result<(), Error> {
        let old_expect_symbol = self.expect_symbol;
        self.expect_symbol = true;
        use serde::Serializer;
        self.serialize_str(v)?;
        self.expect_symbol = old_expect_symbol;
        Ok(())
    }

    fn write_indent(&mut self) -> Result<(), Error> {
        write!(self.writer, "{:width$}", "", width = self.current_indent)?;
        Ok(())
    }

    fn increase_indent(&mut self, reason: &str) {
        self.current_indent += self.indent;
        self.path.push(reason.to_string());
    }

    fn decrease_indent(&mut self) {
        self.path.pop();
        self.current_indent = self.current_indent.saturating_sub(self.indent);
    }
}

#[derive(Debug)]
pub enum Error {
    Message(String),
    Io(std::io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Message(msg) => write!(f, "{}", msg),
            Error::Io(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl<'a, W: Write> serde::Serializer for &'a mut Json5Serializer<W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SerializeVec<'a, W>;
    type SerializeTuple = SerializeVec<'a, W>;
    type SerializeTupleStruct = SerializeVec<'a, W>;
    type SerializeTupleVariant = SerializeTupleVariant<'a, W>;
    type SerializeMap = SerializeMap<'a, W>;
    type SerializeStruct = SerializeMap<'a, W>;
    type SerializeStructVariant = SerializeStructVariant<'a, W>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        match v {
            true => write!(self.writer, "true")?,
            false => write!(self.writer, "false")?,
        }
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "{}", v)?;
        Ok(())
    }

    /// Serialize either symbols or strings. If expect_symbol, consider not quoting.
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        if self.expect_symbol {
            self.expect_symbol = false;
            if property_key_needs_quoting(v) {
                // Fall back to string instead of symbol.
                serialize_string(&mut self.writer, v)?;
            } else {
                write!(self.writer, "{}", v)?;
            }
        } else {
            serialize_string(&mut self.writer, v)?;
        }
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        let s = v.to_string();
        self.serialize_str(&s)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        use serde::ser::SerializeSeq;
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for e in v {
            seq.serialize_element(e)?;
        }
        seq.end()
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        self.serialize_unit()
    }

    fn serialize_some<T: ?Sized>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        self.expect_symbol = false;
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        write!(self.writer, "null")?;
        Ok(())
    }

    fn serialize_unit_struct(
        self,
        _name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.expect_symbol = false;
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        self.expect_symbol = false;
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        self.expect_symbol = false;
        writeln!(self.writer, "{{")?;
        self.increase_line_number();
        self.increase_indent(variant);
        self.write_indent()?;
        self.serialize_symbol(variant)?;
        write!(self.writer, ": ")?;
        value.serialize(&mut *self)?;
        self.decrease_indent();
        writeln!(self.writer)?;
        self.increase_line_number();
        self.write_indent()?;
        write!(self.writer, "}}")?;
        Ok(())
    }

    fn serialize_seq(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        self.expect_symbol = false;
        SerializeVec::new(self)
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        self.expect_symbol = false;
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.expect_symbol = false;
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.expect_symbol = false;
        SerializeTupleVariant::new(self, variant)
    }

    fn serialize_map(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeMap, Self::Error> {
        self.expect_symbol = false;
        SerializeMap::new_map(self)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.expect_symbol = false;
        SerializeMap::new_struct(self, name)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.expect_symbol = false;
        SerializeStructVariant::new(self, variant)
    }
}

pub struct SerializeVec<'a, W: 'a + Write> {
    serializer: &'a mut Json5Serializer<W>,
    first: bool,
}

pub struct SerializeMap<'a, W: 'a + Write> {
    serializer: &'a mut Json5Serializer<W>,
    first: bool,
}

pub struct SerializeTupleVariant<'a, W: 'a + Write> {
    serializer: &'a mut Json5Serializer<W>,
    first: bool,
}

pub struct SerializeStructVariant<'a, W: 'a + Write> {
    serializer: &'a mut Json5Serializer<W>,
    first: bool,
}

impl<'a, W: Write> ser::SerializeSeq for SerializeVec<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        if !self.first {
            writeln!(self.serializer.writer, ",")?;
            self.serializer.increase_line_number();
        }
        self.first = false;
        self.serializer.write_indent()?;
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.serializer.decrease_indent();
        writeln!(self.serializer.writer)?;
        self.serializer.increase_line_number();
        self.serializer.write_indent()?;
        write!(self.serializer.writer, "]")?;
        Ok(())
    }
}

impl<'a, W: Write> SerializeVec<'a, W> {
    fn new(serializer: &'a mut Json5Serializer<W>) -> Result<Self, Error> {
        writeln!(serializer.writer, "[")?;
        serializer.increase_line_number();
        serializer.increase_indent("");
        Ok(Self { serializer, first: true })
    }
}

impl<'a, W: Write> ser::SerializeTuple for SerializeVec<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W: Write> ser::SerializeTupleStruct for SerializeVec<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W: Write> ser::SerializeTupleVariant for SerializeTupleVariant<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        if !self.first {
            write!(self.serializer.writer, ", ")?;
        }
        self.first = false;
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.serializer.decrease_indent();
        writeln!(self.serializer.writer)?;
        self.serializer.increase_line_number();
        self.serializer.write_indent()?;
        write!(self.serializer.writer, "]")?;
        self.serializer.decrease_indent();
        writeln!(self.serializer.writer)?;
        self.serializer.increase_line_number();
        self.serializer.write_indent()?;
        write!(self.serializer.writer, "}}")?;
        Ok(())
    }
}

impl<'a, W: Write> SerializeStructVariant<'a, W> {
    fn new(
        serializer: &'a mut Json5Serializer<W>,
        variant: &str,
    ) -> Result<Self, Error> {
        writeln!(serializer.writer, "{{")?;
        serializer.increase_line_number();
        serializer.increase_indent(variant);
        serializer.write_indent()?;
        serializer.serialize_symbol(variant)?;
        writeln!(serializer.writer, ": {{")?;
        serializer.increase_line_number();
        serializer.increase_indent(variant);
        Ok(Self { serializer, first: true })
    }
}

impl<'a, W: Write> SerializeTupleVariant<'a, W> {
    fn new(
        serializer: &'a mut Json5Serializer<W>,
        variant: &str,
    ) -> Result<Self, Error> {
        writeln!(serializer.writer, "{{")?;
        serializer.increase_line_number();
        serializer.increase_indent(variant);
        serializer.write_indent()?;
        serializer.serialize_symbol(variant)?;
        write!(serializer.writer, ": [")?;
        serializer.increase_indent(variant);
        Ok(Self { serializer, first: true })
    }
}

/// Because map keys can be anything, we need something that allows us to decide whether we need to quote symbols or not

impl<'a, W: Write> ser::SerializeMap for SerializeMap<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        if !self.first {
            writeln!(self.serializer.writer, ",")?;
            self.serializer.increase_line_number();
        }
        self.first = false;
        self.serializer.write_indent()?;

        let old_expect_symbol = self.serializer.expect_symbol;
        self.serializer.expect_symbol = true;
        key.serialize(&mut *self.serializer)?;
        self.serializer.expect_symbol = old_expect_symbol;
        write!(self.serializer.writer, ": ")?;
        Ok(())
    }

    fn serialize_value<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.serializer.decrease_indent();
        writeln!(self.serializer.writer)?;
        self.serializer.increase_line_number();
        self.serializer.write_indent()?;
        write!(self.serializer.writer, "}}")?;
        Ok(())
    }
}

impl<'a, W: Write> SerializeMap<'a, W> {
    fn new_map(serializer: &'a mut Json5Serializer<W>) -> Result<Self, Error> {
        writeln!(serializer.writer, "{{")?;
        serializer.increase_line_number();
        serializer.increase_indent("");
        Ok(Self { serializer, first: true })
    }

    fn new_struct(
        serializer: &'a mut Json5Serializer<W>,
        name: &str,
    ) -> Result<Self, Error> {
        // almost self.serialize_map(Some(len))
        writeln!(serializer.writer, "{{")?;
        serializer.increase_line_number();
        serializer.increase_indent(name);
        Ok(SerializeMap { serializer, first: true })
    }
}

impl<'a, W: Write> ser::SerializeStruct for SerializeMap<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        // Inlined specialization of ser::SerializeMap::serialize_key(self, key: &str):

        if !self.first {
            writeln!(self.serializer.writer, ",")?;
            self.serializer.increase_line_number();
        }
        self.first = false;
        self.serializer.write_indent()?;
        self.serializer.serialize_symbol(key)?;
        write!(self.serializer.writer, ": ")?;
        ser::SerializeMap::serialize_value(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeMap::end(self)
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        eprintln!(
            "{}: skipped field {} around JSON5 line number {}",
            self.serializer.path.join("/"),
            key,
            self.serializer.line_number
        );
        Ok(())
    }
}

impl<'a, W: Write> ser::SerializeStructVariant
    for SerializeStructVariant<'a, W>
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        if !self.first {
            writeln!(self.serializer.writer, ",")?;
            self.serializer.increase_line_number();
        }
        self.first = false;
        self.serializer.write_indent()?;
        self.serializer.serialize_symbol(key)?;
        write!(self.serializer.writer, ": ")?;
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.serializer.decrease_indent();
        writeln!(self.serializer.writer)?;
        self.serializer.increase_line_number();
        self.serializer.write_indent()?;
        write!(self.serializer.writer, "}}")?;
        self.serializer.decrease_indent();
        writeln!(self.serializer.writer)?;
        self.serializer.increase_line_number();
        self.serializer.write_indent()?;
        write!(self.serializer.writer, "}}")?;
        Ok(())
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        eprintln!(
            "{}: skipped field {} around JSON5 line number {}",
            self.serializer.path.join("/"),
            key,
            self.serializer.line_number
        );
        Ok(())
    }
}

pub fn to_writer<W, T>(writer: W, value: &T) -> Result<(), Error>
where
    W: Write,
    T: ?Sized + Serialize,
{
    let mut serializer = Json5Serializer::new(writer);
    value.serialize(&mut serializer)?;
    Ok(())
}

// By convention, the public API of a Serde serializer is one or more `to_abc`
// functions such as `to_string`, `to_bytes`, or `to_writer` depending on what
// Rust types the serializer is able to produce as output.
pub fn to_string_pretty<T>(value: &T) -> Result<String, Error>
where
    T: Serialize,
{
    let mut buffer = Vec::new();
    to_writer(&mut buffer, value)?;
    let result = String::from_utf8(buffer).unwrap();
    Ok(result)
}

#[test]
fn test_struct() {
    use serde::Serialize;
    #[derive(Serialize)]
    struct Test {
        #[serde(skip_serializing_if = "Option::is_none")]
        a: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        b: Option<u32>,
    }

    let test = Test { a: None, b: Some(1) };
    let json5_string = to_string_pretty(&test).unwrap();
    eprintln!("{}", json5_string);
    assert!(!json5_string.contains('a'));
    assert!(json5_string.contains('b'));
}
