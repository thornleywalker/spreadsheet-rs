use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use crate::{
    cell::{CellIdx, Value},
    sheet::{Sheet, SheetIdx, ValueResult, ValueSource},
};

pub mod cell;
pub mod formula;
mod language;
pub mod sheet;

// Potential configuration, used it for a bit, but nothing currently, but might still later
#[derive(Debug)]
struct Config {}

#[derive(Debug)]
pub enum Error {
    MaxRecursionReached,
    CyclicDependencyDetected,
    InconsistentCaching,
    RefMustBeNumber,
    OperationUnavailable,
    FunctionNotAvailable,
    DivideByZero,
    InsufficientArgs,
    TooManyArgs,
    SheetNotFound,
}

#[derive(Debug)]
enum CacheEntry {
    /// The entry has initated calculation, but has not yet completed.
    /// Pulling this value from the cache indicates a cyclic dependency
    Calculating,
    /// The value has been calculated
    Calculated(Value),
}

#[derive(Debug)]
pub struct Spanleaf {
    sheets: BTreeMap<SheetIdx, Sheet>,

    // could probably refactor into a Cache type for convenience
    /// Cache of values to reduce duplicate calculation and detect cyclic dependencies
    cache: RefCell<BTreeMap<(SheetIdx, CellIdx), CacheEntry>>,
    /// Chain of dependencies, where the key is the dependee, and the value is a set of dependents
    dependencies: RefCell<BTreeMap<(SheetIdx, CellIdx), BTreeSet<(SheetIdx, CellIdx)>>>,

    _config: Config,
}
impl Spanleaf {
    pub fn new() -> Self {
        Self {
            _config: Config {},

            sheets: Default::default(),
            cache: Default::default(),
            dependencies: Default::default(),
        }
    }

    /// Inserts a new sheet to the Spanleaf
    ///
    /// Because this is the only way to get a sheet index, we can know that it'll be present
    pub fn insert_sheet(&mut self, name: impl ToString) -> SheetIdx {
        let sref = SheetIdx::next();
        self.sheets.insert(sref, Sheet::new(name));
        sref
    }

    /// Inserts a row default to the specified sheet
    pub fn insert_row_default<T: TryInto<Value>>(
        &mut self,
        sheet: SheetIdx,
        row: u64,
        val: T,
    ) -> Result<Value, T::Error> {
        // clear cache for dependents
        let to_clear = {
            let deps = self.dependencies.borrow();
            deps.iter()
                .filter_map(|((sref, cref), v)| (sref == &sheet && cref.row == row).then_some(v))
                .flat_map(|dependants| dependants.iter().cloned())
                .collect::<Vec<_>>()
        };

        for dep in to_clear {
            self.clear_from_cache(dep.0, dep.1);
        }

        Ok(self
            .sheets
            .get_mut(&sheet)
            .map(|s| s.insert_row_default(row, val))
            .transpose()?
            .unwrap_or_default())
    }

    /// Inserts a col default to the specified sheet
    pub fn insert_col_default<T: TryInto<Value>>(
        &mut self,
        sheet: SheetIdx,
        col: u64,
        val: T,
    ) -> Result<Value, T::Error> {
        // clear cache for dependents
        let to_clear = {
            let deps = self.dependencies.borrow();
            deps.iter()
                .filter_map(|((sref, cref), v)| (sref == &sheet && cref.col == col).then_some(v))
                .flat_map(|dependants| dependants.iter().cloned())
                .collect::<Vec<_>>()
        };

        for dep in to_clear {
            self.clear_from_cache(dep.0, dep.1);
        }

        Ok(self
            .sheets
            .get_mut(&sheet)
            .map(|s| s.insert_col_default(col, val))
            .transpose()?
            .unwrap_or_default())
    }

    /// Insert a value to the specified sheet
    pub fn insert<T: TryInto<Value>>(
        &mut self,
        sheet: SheetIdx,
        cref: CellIdx,
        val: T,
    ) -> Result<Value, T::Error> {
        // clear the cache for dependents
        self.clear_from_cache(sheet, cref);

        Ok(self
            .sheets
            .get_mut(&sheet)
            .map(|s| s.insert(cref, val))
            .transpose()?
            .unwrap_or_default())
    }

    pub fn clear_from_cache(&self, sref: SheetIdx, cref: CellIdx) {
        // scope to drop the borrow
        let _maybe_e = { self.cache.borrow_mut().remove(&(sref, cref)) };

        // scope to drop the borrow
        let maybe_deps = { self.dependencies.borrow_mut().remove(&(sref, cref)) };

        if let Some(deps) = maybe_deps {
            for dep in deps {
                self.clear_from_cache(dep.0, dep.1);
            }
        }
    }

    /// Gets and caches the calculated value for the given cell
    pub fn get(&self, sref: SheetIdx, cref: CellIdx) -> Result<ValueResult, Error> {
        let mut val_res = self.get_raw_value(sref, cref);

        // if it's a formula, resolve it recursively to a value
        if let Value::Formula(f) = val_res.as_ref() {
            *val_res = if let Some(cached) = self.cache.borrow().get(&(sref, cref)) {
                // check the cache
                match cached {
                    CacheEntry::Calculating => return Err(Error::CyclicDependencyDetected),
                    CacheEntry::Calculated(value) => value.clone(),
                }
            } else {
                // set cycle trap
                self.cache
                    .borrow_mut()
                    .insert((sref, cref), CacheEntry::Calculating);

                let mut deps = vec![];
                // calculate and cache
                let res = f.eval(self, sref, cref, &mut deps)?;
                // establish the dependency
                for dep in deps {
                    self.dependencies
                        .borrow_mut()
                        .entry(dep)
                        .or_default()
                        .insert((sref, cref));
                }

                // clear cycle trap
                let Some(CacheEntry::Calculating) = self
                    .cache
                    .borrow_mut()
                    .insert((sref, cref), CacheEntry::Calculated(res.clone()))
                else {
                    // the trap /should/ be a Some(Calculating), is it possible for this to not be true?
                    return Err(Error::InconsistentCaching);
                };

                res
            };
        };

        Ok(val_res)
    }

    /// Gets the uncalculated value for the given cell
    ///
    /// Useful for formula bar displaying
    pub fn get_raw_value(&self, sref: SheetIdx, cref: CellIdx) -> ValueResult {
        self.sheets
            .get(&sref)
            .map(|s| s.get_formula(cref))
            .unwrap_or_default()
    }

    pub fn get_row_default(&self, sref: SheetIdx, row: u64) -> ValueResult {
        ValueResult {
            value: self
                .sheets
                .get(&sref)
                .map(|s| s.get_row_default(row))
                .unwrap_or_default(),
            source: ValueSource::RowDefault,
        }
    }

    pub fn get_col_default(&self, sref: SheetIdx, col: u64) -> ValueResult {
        ValueResult {
            value: self
                .sheets
                .get(&sref)
                .map(|s| s.get_col_default(col))
                .unwrap_or_default(),
            source: ValueSource::ColDefault,
        }
    }
}

impl Default for Spanleaf {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Spanleaf, cell::CellIdx};

    #[test]
    fn big_test() {
        let mut sl = Spanleaf::new();
        let s0 = sl.insert_sheet("Sheet1");

        sl.insert(s0, CellIdx::new(0, 0), 42.0).unwrap();

        sl.insert(s0, CellIdx::new(0, 1), "Hello World!").unwrap();

        sl.insert(s0, CellIdx::new(0, 2), "=5").unwrap();

        dbg!(&sl);

        dbg!(sl.get(s0, CellIdx::new(0, 0)));
        dbg!(sl.get(s0, CellIdx::new(0, 1)));
        dbg!(sl.get(s0, CellIdx::new(0, 2)));
    }
}
