use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::cell::{CellIdx, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum ValueResult {
    Native(Value),
    RowDefault(Value),
    ColDefault(Value),
}
impl ValueResult {
    pub fn native(val: impl Into<Value>) -> Self {
        Self::Native(val.into())
    }
    pub fn row(val: impl Into<Value>) -> Self {
        Self::RowDefault(val.into())
    }
    pub fn col(val: impl Into<Value>) -> Self {
        Self::ColDefault(val.into())
    }
    pub fn value(self) -> Value {
        self.into()
    }
    pub fn get_mut(&mut self) -> &mut Value {
        match self {
            ValueResult::Native(value)
            | ValueResult::RowDefault(value)
            | ValueResult::ColDefault(value) => value,
        }
    }
    pub fn map(self, f: impl FnOnce(Value) -> Value) -> Self {
        match self {
            ValueResult::Native(value) => ValueResult::Native(f(value)),
            ValueResult::RowDefault(value) => ValueResult::RowDefault(f(value)),
            ValueResult::ColDefault(value) => ValueResult::ColDefault(f(value)),
        }
    }
}
impl Default for ValueResult {
    fn default() -> Self {
        Self::Native(Value::default())
    }
}
impl AsRef<Value> for ValueResult {
    fn as_ref(&self) -> &Value {
        match self {
            ValueResult::Native(value)
            | ValueResult::RowDefault(value)
            | ValueResult::ColDefault(value) => value,
        }
    }
}
impl AsMut<Value> for ValueResult {
    fn as_mut(&mut self) -> &mut Value {
        match self {
            ValueResult::Native(value)
            | ValueResult::RowDefault(value)
            | ValueResult::ColDefault(value) => value,
        }
    }
}
impl Deref for ValueResult {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        match self {
            ValueResult::Native(value)
            | ValueResult::RowDefault(value)
            | ValueResult::ColDefault(value) => value,
        }
    }
}
impl DerefMut for ValueResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            ValueResult::Native(value)
            | ValueResult::RowDefault(value)
            | ValueResult::ColDefault(value) => value,
        }
    }
}
impl From<ValueResult> for Value {
    fn from(value: ValueResult) -> Self {
        match value {
            ValueResult::Native(value)
            | ValueResult::RowDefault(value)
            | ValueResult::ColDefault(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SheetIdx(u64);
impl SheetIdx {
    pub(crate) fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Debug, Clone)]
pub struct Sheet {
    pub name: String,
    // uses shell/rank major order
    cells: BTreeMap<u64, Value>,
    row_defaults: BTreeMap<u64, Value>,
    col_defaults: BTreeMap<u64, Value>,
}
impl Sheet {
    pub fn new(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            cells: Default::default(),
            row_defaults: Default::default(),
            col_defaults: Default::default(),
        }
    }
    /// Returns the previous value
    pub fn insert_row_default<T: TryInto<Value>>(
        &mut self,
        row: u64,
        val: T,
    ) -> Result<Value, T::Error> {
        let val = val.try_into()?;
        if let Value::None = val {
            Ok(self.row_defaults.remove(&row).unwrap_or_default())
        } else {
            Ok(self.row_defaults.insert(row, val).unwrap_or_default())
        }
    }
    /// Returns the previous value
    pub fn insert_col_default<T: TryInto<Value>>(
        &mut self,
        col: u64,
        val: T,
    ) -> Result<Value, T::Error> {
        let val = val.try_into()?;
        if let Value::None = val {
            Ok(self.col_defaults.remove(&col).unwrap_or_default())
        } else {
            Ok(self.col_defaults.insert(col, val).unwrap_or_default())
        }
    }
    /// Returns the previous value
    pub fn insert<T: TryInto<Value>>(&mut self, cref: CellIdx, val: T) -> Result<Value, T::Error> {
        let val = val.try_into()?;
        let offset = cell_ref_to_shell_off(cref);
        if let Value::None = val {
            Ok(self.cells.remove(&offset).unwrap_or_default())
        } else {
            Ok(self.cells.insert(offset, val).unwrap_or_default())
        }
    }
    pub fn get_formula(&self, cref: CellIdx) -> ValueResult {
        let offset = cell_ref_to_shell_off(cref);
        self.cells
            .get(&offset)
            .map(|v| ValueResult::Native(v.clone()))
            .or_else(|| {
                self.col_defaults
                    .get(&cref.col)
                    .map(|v| ValueResult::ColDefault(v.clone()))
            })
            .or_else(|| {
                self.row_defaults
                    .get(&cref.row)
                    .map(|v| ValueResult::RowDefault(v.clone()))
            })
            .unwrap_or_default()
    }
}

/// Converts the row and column to a shell offset
/// ```text
/// a b c d e
/// f g h i j
/// k l m n o
/// p q r s t
/// u v w x y
/// ```
///
/// would be stored like so (parenthesis show the shells)
///
/// ```text
/// [(a) (b g f) (c h m l k) (d i n s r q p) (e j o t y x w v u)]
/// ```
fn cell_ref_to_shell_off(CellIdx { row, col }: CellIdx) -> u64 {
    let max = row.max(col);
    // let rank = max + 1;

    // rank * rank - (rank - r) - c
    (max * max) + max + row - col
}

#[cfg(test)]
mod tests {
    use crate::{
        cell::{CellIdx, Value},
        sheet::{Sheet, ValueResult},
    };

    #[test]
    fn insert_and_get() {
        let mut sheet = Sheet::new("");
        let nil = sheet.insert(CellIdx::new(12, 12), 7).unwrap();
        assert_eq!(nil, ().into());

        let seven = sheet.insert(CellIdx::new(12, 12), 18).unwrap();
        assert_eq!(seven, 7.into());

        let eighteen = sheet.get_formula(CellIdx::new(12, 12));
        assert_eq!(*eighteen, 18.into());
    }

    #[test]
    fn row_col_defaults() {
        let mut sheet = Sheet::new("");
        let row_1 = Value::try_from("row 1").unwrap();
        let col_1 = Value::try_from("col 1").unwrap();
        sheet.insert_row_default(1, row_1.clone()).unwrap();
        sheet.insert_col_default(1, col_1.clone()).unwrap();

        let r0c0 = sheet.get_formula(CellIdx::new(0, 0));
        let r0c1 = sheet.get_formula(CellIdx::new(0, 1));
        let r1c0 = sheet.get_formula(CellIdx::new(1, 0));
        let r1c1 = sheet.get_formula(CellIdx::new(1, 1));

        assert_eq!(r0c0, ValueResult::native(()));
        assert_eq!(r0c1, ValueResult::col(col_1.clone()));
        assert_eq!(r1c0, ValueResult::row(row_1));
        // column default takes priority
        assert_eq!(r1c1, ValueResult::col(col_1));
    }
}
