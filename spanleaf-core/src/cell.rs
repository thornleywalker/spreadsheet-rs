use std::ops;

use crate::{
    Error,
    formula::{Formula, FormulaError},
    sheet::SheetIdx,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellIdx {
    pub row: u64,
    pub col: u64,
}
impl CellIdx {
    pub fn new(row: u64, col: u64) -> Self {
        Self { row, col }
    }
}

#[derive(Debug, Clone, Default)]
pub enum Value {
    #[default]
    None,
    Bool(bool),
    Number(f64),
    String(String),
    // Date(),
    // Array(),
    // Range(),
    Ref {
        sref: SheetIdx,
        cref: CellIdx,
    },
    Formula(Formula),
}
impl Value {
    pub fn new(val: impl Into<Value>) -> Self {
        val.into()
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(l0), Self::Bool(r0)) => l0 == r0,
            (Self::Number(l0), Self::Number(r0)) => l0 == r0,
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            (
                Self::Ref {
                    sref: l_sref,
                    cref: l_cref,
                },
                Self::Ref {
                    sref: r_sref,
                    cref: r_cref,
                },
            ) => l_sref == r_sref && l_cref == r_cref,
            (Self::Formula(_l0), Self::Formula(_r0)) => false,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl From<()> for Value {
    fn from(_value: ()) -> Self {
        Self::None
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl TryFrom<&str> for Value {
    type Error = FormulaError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some('=') = value.chars().next() {
            Ok(Self::Formula(Formula::parse(&value[1..])?))
        } else {
            Ok(Self::String(value.to_string()))
        }
    }
}

impl TryFrom<String> for Value {
    type Error = FormulaError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        TryFrom::try_from(value.as_str())
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}
impl From<f32> for Value {
    fn from(value: f32) -> Self {
        (value as f64).into()
    }
}
impl From<u8> for Value {
    fn from(value: u8) -> Self {
        (value as f64).into()
    }
}
impl From<u16> for Value {
    fn from(value: u16) -> Self {
        (value as f64).into()
    }
}
impl From<u32> for Value {
    fn from(value: u32) -> Self {
        (value as f64).into()
    }
}
impl From<u64> for Value {
    fn from(value: u64) -> Self {
        (value as f64).into()
    }
}
impl From<i8> for Value {
    fn from(value: i8) -> Self {
        (value as f64).into()
    }
}
impl From<i16> for Value {
    fn from(value: i16) -> Self {
        (value as f64).into()
    }
}
impl From<i32> for Value {
    fn from(value: i32) -> Self {
        (value as f64).into()
    }
}
impl From<i64> for Value {
    fn from(value: i64) -> Self {
        (value as f64).into()
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => v.into(),
            None => Value::None,
        }
    }
}

impl ops::Neg for Value {
    type Output = Result<Value, Error>;

    fn neg(self) -> Self::Output {
        match self {
            Value::None => Ok(Value::None),
            Value::Bool(b) => Ok(Value::Bool(!b)),
            Value::Number(f) => Ok(Value::Number(-f)),
            Value::String(_) | Value::Ref { .. } | Value::Formula(_) => {
                Err(Error::OperationUnavailable)
            }
        }
    }
}

impl ops::Add for Value {
    type Output = Result<Value, Error>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::None, other) | (other, Value::None) => Ok(other),
            (Value::Bool(_), Value::Bool(_)) => Err(Error::OperationUnavailable),
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),

            (Value::Formula(_), _)
            | (_, Value::Formula(_))
            | (Value::Ref { .. }, _)
            | (_, Value::Ref { .. })
            | (Value::Bool(_), _)
            | (_, Value::Bool(_))
            | (Value::Number(_), _)
            | (_, Value::Number(_)) => Err(Error::OperationUnavailable),
        }
    }
}

impl ops::Sub for Value {
    type Output = Result<Value, Error>;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::None, other) | (other, Value::None) => Ok(other),
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
            (Value::Bool(_), Value::Bool(_))
            | (Value::String(_), Value::String(_))
            | (Value::Formula(_), _)
            | (_, Value::Formula(_))
            | (Value::Ref { .. }, _)
            | (_, Value::Ref { .. })
            | (Value::Bool(_), _)
            | (_, Value::Bool(_))
            | (Value::Number(_), _)
            | (_, Value::Number(_)) => Err(Error::OperationUnavailable),
        }
    }
}

impl ops::Mul for Value {
    type Output = Result<Value, Error>;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::None, other) | (other, Value::None) => Ok(other),
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
            (Value::Bool(_), Value::Bool(_))
            | (Value::String(_), Value::String(_))
            | (Value::Formula(_), _)
            | (_, Value::Formula(_))
            | (Value::Ref { .. }, _)
            | (_, Value::Ref { .. })
            | (Value::Bool(_), _)
            | (_, Value::Bool(_))
            | (Value::Number(_), _)
            | (_, Value::Number(_)) => Err(Error::OperationUnavailable),
        }
    }
}

impl ops::Div for Value {
    type Output = Result<Value, Error>;

    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::None, other) | (other, Value::None) => Ok(other),
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a / b)),
            (Value::Bool(_), Value::Bool(_))
            | (Value::String(_), Value::String(_))
            | (Value::Formula(_), _)
            | (_, Value::Formula(_))
            | (Value::Ref { .. }, _)
            | (_, Value::Ref { .. })
            | (Value::Bool(_), _)
            | (_, Value::Bool(_))
            | (Value::Number(_), _)
            | (_, Value::Number(_)) => Err(Error::OperationUnavailable),
        }
    }
}
