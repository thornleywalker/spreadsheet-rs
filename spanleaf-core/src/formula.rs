use chumsky::Parser;

use crate::{
    Error, Spanleaf,
    cell::{CellIdx, Value},
    language::{self, Expr},
    sheet::SheetIdx,
};

#[derive(Debug, Clone)]
pub enum FormulaError {
    InvalidFormula,
}

#[derive(Debug, Clone)]
pub struct Formula {
    pub script: String,
    expr: Expr,
}
impl Formula {
    /// Parses the script, returning an error if the script is invalid
    pub fn parse(script: &str) -> Result<Self, FormulaError> {
        let expr = language::parser().parse(script).unwrap();
        Ok(Formula {
            script: script.to_string(),
            expr,
        })
    }
    /// Evaluate the formula
    pub(crate) fn eval(
        &self,
        sl: &mut Spanleaf,
        curr_sheet: SheetIdx,
        dependencies: &mut Vec<(SheetIdx, CellIdx)>,
        rec_lvl: usize,
    ) -> Result<Value, Error> {
        language::eval(
            &self.expr,
            &mut language::EvalCtx {
                sl,
                curr_sheet,
                dependencies,
            },
        )
    }
}
