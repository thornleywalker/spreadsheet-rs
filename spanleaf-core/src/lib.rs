use std::collections::{BTreeMap, BTreeSet};

use crate::{
    cell::{CellIdx, Value},
    sheet::{Sheet, SheetIdx, ValueResult},
};

pub mod cell;
pub mod formula;
mod language;
pub mod sheet;

#[derive(Debug)]
struct Config {
    max_recursion: usize,
}

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

    cache: BTreeMap<(SheetIdx, CellIdx), CacheEntry>,
    /// Chain of dependencies, where the key is the dependee, and the value is a set of dependents
    dependencies: BTreeMap<(SheetIdx, CellIdx), BTreeSet<(SheetIdx, CellIdx)>>,

    // parser: Box<dyn Parser<'src, &'src str, Expr>>,
    config: Config,
}
impl Spanleaf {
    pub fn new() -> Self {
        Self {
            config: Config { max_recursion: 32 },

            sheets: Default::default(),
            cache: Default::default(),
            dependencies: Default::default(),
            // parser: Box::new(parser),
        }
    }

    pub fn insert_sheet(&mut self, name: impl ToString) -> SheetIdx {
        let sref = SheetIdx::next();
        self.sheets.insert(sref, Sheet::new(name));
        sref
    }

    pub fn insert_row_default<T: TryInto<Value>>(
        &mut self,
        sheet: SheetIdx,
        row: u64,
        val: T,
    ) -> Result<Value, T::Error> {
        // clear cache for dependents
        todo!();

        Ok(self
            .sheets
            .get_mut(&sheet)
            .map(|s| s.insert_row_default(row, val))
            .transpose()?
            .unwrap_or_default())
    }

    pub fn insert_col_default<T: TryInto<Value>>(
        &mut self,
        sheet: SheetIdx,
        col: u64,
        val: T,
    ) -> Result<Value, T::Error> {
        // clear cache for dependents
        todo!();

        Ok(self
            .sheets
            .get_mut(&sheet)
            .map(|s| s.insert_col_default(col, val))
            .transpose()?
            .unwrap_or_default())
    }

    pub fn insert<T: TryInto<Value>>(
        &mut self,
        sheet: SheetIdx,
        cref: CellIdx,
        val: T,
    ) -> Result<Value, T::Error> {
        // clear the cache for dependents
        if let Some(deps) = self.dependencies.get(&(sheet, cref)) {
            for dependent in deps {
                self.cache.remove(dependent);
            }
        }

        Ok(self
            .sheets
            .get_mut(&sheet)
            .map(|s| s.insert(cref, val))
            .transpose()?
            .unwrap_or_default())
    }

    pub fn get(&mut self, sref: SheetIdx, cref: CellIdx) -> Result<ValueResult, Error> {
        self.get_rec(sref, cref, 0)
    }

    /// Gets and caches the calculated value for the given cell
    pub(crate) fn get_rec(
        &mut self,
        sref: SheetIdx,
        cref: CellIdx,
        rec_lvl: usize,
    ) -> Result<ValueResult, Error> {
        if rec_lvl >= self.config.max_recursion {
            return Err(Error::MaxRecursionReached);
        }

        let mut val_res = self.get_raw_value(sref, cref);

        // if it's a formula, resolve it recursively to a value
        if let Value::Formula(f) = val_res.as_ref() {
            *val_res = if let Some(cached) = self.cache.get(&(sref, cref)) {
                // check the cache
                match cached {
                    CacheEntry::Calculating => return Err(Error::CyclicDependencyDetected),
                    CacheEntry::Calculated(value) => value.clone(),
                }
            } else {
                // set cycle trap
                self.cache.insert((sref, cref), CacheEntry::Calculating);

                let mut deps = vec![];
                // calculate and cache
                let res = f.eval(self, sref, &mut deps, rec_lvl)?;
                // establish the dependency
                for dep in deps {
                    self.dependencies
                        .entry(dep)
                        .or_default()
                        .insert((sref, cref));
                }

                // clear cycle trap
                let Some(CacheEntry::Calculating) = self
                    .cache
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
    pub fn get_raw_value(&self, sref: SheetIdx, cref: CellIdx) -> ValueResult {
        self.sheets
            .get(&sref)
            .map(|s| s.get_formula(cref))
            .unwrap_or_default()
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
