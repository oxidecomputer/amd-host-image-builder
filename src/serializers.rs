use serde::{ser, Serialize};
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use std::result::Result;

// TODO: maybe use a 'proxy' approach like serde path-to-error. "trigger" will actually show the path.

pub(crate) struct PathNode {
    text: String,
    next: Option<Rc<PathNode>>,
}
impl PathNode {
    pub(crate) fn append(
        next: Option<Rc<Self>>,
        text: String,
    ) -> Option<Rc<Self>> {
        Some(Rc::new(Self { text, next }))
    }
}
impl std::fmt::Display for PathNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.next {
            Some(ref x) => {
                x.fmt(f)?;
                write!(f, "/")?;
            }
            None => {}
        }
        write!(f, "{}", self.text)
    }
}

/// This serializer actually doesn't have an output.
/// Its purpose is to notice when fields are skipped.
/// Since our Permissive Serializers skip fields on error, that means
/// that we encountered a raw value we don't know while serializing.
/// This serializer here makes sure to log the path to that raw value
/// on standard error.
pub(crate) struct DummySerializer {
    pub(crate) path: Option<Rc<PathNode>>,
}

#[derive(Debug)]
pub struct Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl ser::StdError for Error {}

impl ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        todo!()
    }
}

type Ok = ();

pub struct SerializeVec {
    pub(crate) path: Option<Rc<PathNode>>,
}

impl serde::ser::SerializeSeq for SerializeVec {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: self.path.clone(), // TODO: Count elements or something
        })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTupleStruct for SerializeVec {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<(), Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl serde::ser::SerializeTuple for SerializeVec {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<(), Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

pub struct SerializeMap {
    pub(crate) path: Option<Rc<PathNode>>,
}

impl serde::ser::SerializeMap for SerializeMap {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut DummySerializer {
            path: self.path.clone(), // TODO: maybe more complicated keys ?
        })
    }
    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: self.path.clone(), // TODO: maybe more complicated values ?
        })
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeStruct for SerializeMap {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: PathNode::append(self.path.clone(), key.to_string()),
        })
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        eprintln!(
            "{}: skipped field {}",
            match self.path {
                Some(ref x) => {
                    x.to_string()
                }
                None => {
                    "".to_string()
                }
            },
            key
        );
        Ok(())
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

// TODO SerializeStructVariant

pub struct SerializeTupleVariant {
    pub(crate) path: Option<Rc<PathNode>>,
}

impl serde::ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: self.path.clone(), // TODO: tuple variant
        })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct SerializeStructVariant {
    name: String,
    pub(crate) path: Option<Rc<PathNode>>,
}

impl serde::ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: PathNode::append(self.path.clone(), name.to_string()),
        })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> ser::Serializer for &'a mut DummySerializer {
    type Ok = Ok;
    type Error = Error;
    type SerializeSeq = SerializeVec;
    type SerializeTuple = SerializeVec;
    type SerializeTupleStruct = SerializeVec;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_some<T: ?Sized>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_seq(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SerializeVec {
            path: self.path.clone(), // FIXME
        })
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(SerializeTupleVariant {
            // name: String::from(variant),
            path: self.path.clone(), // FIXME name variant
        })
    }

    fn serialize_map(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeMap, Self::Error> {
        Ok(SerializeMap {
            path: self.path.clone(), // FIXME check
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        // TODO: More features
        Ok(SerializeMap {
            path: PathNode::append(self.path.clone(), name.to_string()),
        })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(SerializeStructVariant {
            name: String::from(variant),
            path: PathNode::append(
                PathNode::append(self.path.clone(), name.to_string()),
                variant.to_string(),
            ),
        })
    }
}

impl<'a> ser::SerializeSeq for &'a mut DummySerializer {
    // Must match the `Ok` type of the serializer.
    type Ok = Ok;
    // Must match the `Error` type of the serializer.
    type Error = Error;

    fn serialize_element<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: self.path.clone(), // FIXME check
        })?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeTuple for &'a mut DummySerializer {
    // Must match the `Ok` type of the serializer.
    type Ok = Ok;
    // Must match the `Error` type of the serializer.
    type Error = Error;

    fn serialize_element<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl<'a> ser::SerializeTupleStruct for &'a mut DummySerializer {
    // Must match the `Ok` type of the serializer.
    type Ok = Ok;
    // Must match the `Error` type of the serializer.
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl<'a> ser::SerializeTupleVariant for &'a mut DummySerializer {
    // Must match the `Ok` type of the serializer.
    type Ok = Ok;
    // Must match the `Error` type of the serializer.
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: self.path.clone(), // FIXME check
        })?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeMap for &'a mut DummySerializer {
    // Must match the `Ok` type of the serializer.
    type Ok = Ok;
    // Must match the `Error` type of the serializer.
    type Error = Error;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        key.serialize(&mut DummySerializer { path: self.path.clone() })?;
        Ok(())
    }

    fn serialize_value<T: ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: self.path.clone(), // FIXME check
        })?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeStruct for &'a mut DummySerializer {
    // Must match the `Ok` type of the serializer.
    type Ok = Ok;
    // Must match the `Error` type of the serializer.
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: PathNode::append(self.path.clone(), key.to_string()),
        })?;
        Ok(())
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        eprintln!(
            "{}: skipped field {}",
            match self.path {
                Some(ref x) => {
                    x.to_string()
                }
                None => {
                    "".to_string()
                }
            },
            key
        );
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeStructVariant for &'a mut DummySerializer {
    // Must match the `Ok` type of the serializer.
    type Ok = Ok;
    // Must match the `Error` type of the serializer.
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        value.serialize(&mut DummySerializer {
            path: PathNode::append(self.path.clone(), key.to_string()),
        })?;
        Ok(())
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        eprintln!(
            "{}: skipped field {}",
            match self.path {
                Some(ref x) => {
                    x.to_string()
                }
                None => {
                    "".to_string()
                }
            },
            key
        );
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

// By convention, the public API of a Serde serializer is one or more `to_abc`
// functions such as `to_string`, `to_bytes`, or `to_writer` depending on what
// Rust types the serializer is able to produce as output.
//
// This basic serializer supports only `to_string`.
fn to_string<T>(value: &T)
where
    T: Serialize,
{
    let mut serializer = DummySerializer { path: None };
    value.serialize(&mut serializer).unwrap();
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
    to_string(&test);
}

// See serde_test maybe
