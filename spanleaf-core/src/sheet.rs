use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::cell::{CellIdx, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum ValueSource {
    Native,
    RowDefault,
    ColDefault,
}

/// The result of a value fetch from the sheet. Contains metadata about where the Value came from
#[derive(Debug, Clone, PartialEq)]
pub struct ValueResult {
    pub value: Value,
    pub source: ValueSource,
}
impl ValueResult {
    pub fn new(val: impl Into<Value>, source: ValueSource) -> Self {
        Self {
            value: val.into(),
            source,
        }
    }
    pub fn native(val: impl Into<Value>) -> Self {
        Self::new(val, ValueSource::Native)
    }
    pub fn row(val: impl Into<Value>) -> Self {
        Self::new(val, ValueSource::RowDefault)
    }
    pub fn col(val: impl Into<Value>) -> Self {
        Self::new(val, ValueSource::ColDefault)
    }
    pub fn value(self) -> Value {
        self.into()
    }
    pub fn get_mut(&mut self) -> &mut Value {
        &mut self.value
    }
    pub fn map(mut self, f: impl FnOnce(Value) -> Value) -> Self {
        self.value = f(self.value);
        self
    }
}
impl Default for ValueResult {
    fn default() -> Self {
        Self::native(Value::default())
    }
}
impl AsRef<Value> for ValueResult {
    fn as_ref(&self) -> &Value {
        &self.value
    }
}
impl AsMut<Value> for ValueResult {
    fn as_mut(&mut self) -> &mut Value {
        &mut self.value
    }
}
impl Deref for ValueResult {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl DerefMut for ValueResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
impl From<ValueResult> for Value {
    fn from(value: ValueResult) -> Self {
        value.value
    }
}

/// The internal index of a sheet. Atomically incremented when created
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SheetIdx(u64);
impl SheetIdx {
    pub(crate) fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

/// A sheet of values
///
/// Theoretically infinite, as any value not explicitly present still exists as a [Value::None]
///
/// Allows for specifying of a default value for a given row or column, which is what gets returned if the
/// specified value is None, aka the cell is empty. Priority is native value, column default, then row default
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
    /// Inserts a new default value for a row
    ///
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
    /// Inserts a new default value for a col
    ///
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
    /// Inserts a new value into the sheet
    ///
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
    /// Returns the raw, uncalculated formula at the given index
    pub fn get_formula(&self, cref: CellIdx) -> ValueResult {
        let offset = cell_ref_to_shell_off(cref);
        self.cells
            .get(&offset)
            .map(|v| ValueResult::native(v.clone()))
            .or_else(|| {
                self.col_defaults
                    .get(&cref.col)
                    .map(|v| ValueResult::col(v.clone()))
            })
            .or_else(|| {
                self.row_defaults
                    .get(&cref.row)
                    .map(|v| ValueResult::row(v.clone()))
            })
            .unwrap_or_default()
    }

    pub fn get_row_default(&self, row: u64) -> Value {
        self.row_defaults.get(&row).cloned().unwrap_or_default()
    }

    pub fn get_col_default(&self, col: u64) -> Value {
        self.col_defaults.get(&col).cloned().unwrap_or_default()
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
///
/// A significant amount of the time, spreadsheets really only use a very small number of rows and columns
/// (when was the last time you referenced cell Z105?) so row major and col major both feel like inept tradeoffs
///
/// Shell (or rank?) major order prioritizes elements close to the origin, and can be expanded
/// without major reallocations
///
/// Ultimately, we're storing things in a BTree to account for probable sparsity of the matrix,
/// with the shell offset as the index, so really this is just because I think it's interesting
fn cell_ref_to_shell_off(CellIdx { row, col }: CellIdx) -> u64 {
    let max = row.max(col);
    // let rank = max + 1;

    // rank * rank - (rank - r) - c
    // simplifies down to
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
