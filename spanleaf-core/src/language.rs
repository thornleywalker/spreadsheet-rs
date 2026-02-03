use std::{num::ParseFloatError, ops};

use chumsky::{number, prelude::*};

use crate::{
    Error, Spanleaf,
    cell::{CellIdx, Value},
    sheet::{SheetIdx, ValueResult},
};

#[derive(Debug, Clone)]
pub(super) enum Expr {
    Number(f64),
    String(String),
    Bool(bool),
    Sheet(String),
    CellRef(Option<Box<Expr>>, Box<Expr>, Box<Expr>),
    CellDeref(Box<Expr>),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    /// Fn name, then arguments list
    Call(String, Vec<Expr>),
}
impl Expr {
    pub fn cell_ref(sref: Option<Expr>, row: Expr, col: Expr) -> Self {
        Self::CellRef(sref.map(Box::new), Box::new(row), Box::new(col))
    }
    pub fn cell_deref(cref: Expr) -> Self {
        Self::CellDeref(Box::new(cref))
    }
    pub fn neg(expr: Expr) -> Self {
        Self::Neg(Box::new(expr))
    }
    pub fn add(lhs: Expr, rhs: Expr) -> Self {
        Self::Add(Box::new(lhs), Box::new(rhs))
    }
    pub fn sub(lhs: Expr, rhs: Expr) -> Self {
        Self::Sub(Box::new(lhs), Box::new(rhs))
    }
    pub fn mul(lhs: Expr, rhs: Expr) -> Self {
        Self::Mul(Box::new(lhs), Box::new(rhs))
    }
    pub fn div(lhs: Expr, rhs: Expr) -> Self {
        Self::Div(Box::new(lhs), Box::new(rhs))
    }
}

/// takes the function meat (sans '=') and parses it into an expression
pub(crate) fn parser<'src>() -> impl Parser<'src, &'src str, Expr> {
    let expr = recursive({
        |expr| {
            let num = number::number::<{ number::format::STANDARD }, &str, f64, extra::Default>()
                .map(Expr::Number)
                .padded();

            let string = any()
                .filter(|c| c != &'\'')
                .repeated()
                .collect::<String>()
                .map(Expr::String);

            let boolean = just("true")
                .or(just("false"))
                .map(|s| Expr::Bool(s.parse().unwrap()));

            let ident = text::ascii::ident().padded();

            let call = ident
                .then(
                    expr.clone()
                        .separated_by(just(','))
                        .allow_trailing()
                        .collect::<Vec<Expr>>()
                        .delimited_by(just('('), just(')')),
                )
                .map(|(name, args): (&str, _)| Expr::Call(name.to_string(), args));

            let raw_ref = ident
                .or_not()
                .map(move |sheet_name| sheet_name.map(|sn: &str| Expr::Sheet(sn.to_string())))
                .then(
                    expr.clone()
                        .then(just(','))
                        .then(expr.clone())
                        .delimited_by(just('['), just(']')),
                );

            let deref = raw_ref
                .clone()
                .map(|(sheet, ((row, _), col))| Expr::cell_deref(Expr::cell_ref(sheet, row, col)));

            let cref = just('&')
                .then(raw_ref)
                .map(|(_, (sheet, ((row, _), col)))| Expr::cell_ref(sheet, row, col));

            let atom = choice((
                num,
                boolean,
                expr.delimited_by(just('('), just(')')),
                string.delimited_by(just('\''), just('\'')),
                call,
                cref,
                deref,
            ))
            .padded();

            let op = |c| just(c).padded();

            let unary = op('-').repeated().foldr(atom, |_op, rhs| Expr::neg(rhs));

            let product = unary.clone().foldl(
                choice((
                    op('*').to(Expr::mul as fn(_, _) -> _),
                    op('/').to(Expr::div as fn(_, _) -> _),
                ))
                .then(unary)
                .repeated(),
                |lhs, (op, rhs)| op(lhs, rhs),
            );

            let sum = product.clone().foldl(
                choice((
                    op('+').to(Expr::add as fn(_, _) -> _),
                    op('-').to(Expr::sub as fn(_, _) -> _),
                ))
                .then(product)
                .repeated(),
                |lhs, (op, rhs)| op(lhs, rhs),
            );

            sum
        }
    });

    expr
}

pub struct EvalCtx<'a> {
    pub sl: &'a mut Spanleaf,
    pub curr_sheet: SheetIdx,
    pub dependencies: &'a mut Vec<(SheetIdx, CellIdx)>,
}

pub fn eval(expr: &Expr, ctx: &mut EvalCtx<'_>) -> Result<Value, Error> {
    match expr {
        Expr::Number(f) => Ok(Value::Number(*f)),
        Expr::String(s) => Ok(Value::String(s.clone())),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Sheet(name) => Ok(Value::String(name.clone())),
        Expr::CellRef(sheet_ref, row, col) => {
            let Value::Number(row) = eval(row, ctx)? else {
                return Err(Error::RefMustBeNumber);
            };
            let Value::Number(col) = eval(col, ctx)? else {
                return Err(Error::RefMustBeNumber);
            };

            let sref = match sheet_ref {
                Some(sheet_ref) => {
                    let Value::String(sheet_name) = eval(sheet_ref, ctx)? else {
                        return Err(Error::RefMustBeNumber);
                    };

                    ctx.sl
                        .sheets
                        .iter()
                        .find_map(|(k, v)| (v.name == sheet_name).then_some(*k))
                        .ok_or(Error::SheetNotFound)?
                }
                None => ctx.curr_sheet,
            };

            let cref = CellIdx::new(row as u64, col as u64);

            ctx.dependencies.push((sref, cref));

            Ok(Value::Ref { sref, cref })
        }
        Expr::CellDeref(cref) => {
            let Value::Ref { sref, cref } = eval(cref, ctx)? else {
                return Err(Error::RefMustBeNumber);
            };

            ctx.sl.get(sref, cref).map(ValueResult::value)
        }
        Expr::Neg(expr) => Ok(ops::Neg::neg(eval(expr, ctx)?)?),
        Expr::Add(lhs, rhs) => Ok(ops::Add::add(eval(lhs, ctx)?, eval(rhs, ctx)?)?),
        Expr::Sub(lhs, rhs) => Ok(ops::Sub::sub(eval(lhs, ctx)?, eval(rhs, ctx)?)?),
        Expr::Mul(lhs, rhs) => Ok(ops::Mul::mul(eval(lhs, ctx)?, eval(rhs, ctx)?)?),
        Expr::Div(lhs, rhs) => Ok(ops::Div::div(eval(lhs, ctx)?, eval(rhs, ctx)?)?),
        Expr::Call(fn_name, args) => {
            // I don't want to create exprs for every action, that sounds like a nightmare. So I think just an enum and associated functions? Maybe not even an enum?
            // Can also create a HashMap<String, fn(&Expr) -> Result<Value, Error>> to make it more dynamic friendly, populate it on startup or use statics?
            match fn_name.as_str() {
                "sum" => functions::sum(ctx, args),
                "average" => functions::average(ctx, args),
                _ => Err(Error::FunctionNotAvailable),
            }
        }
    }
}

mod functions {
    pub use info::*;
    pub use logical::*;
    pub use math::*;
    pub use statistical::*;

    mod info {
        use crate::{
            Error,
            cell::Value,
            language::{EvalCtx, Expr, eval},
        };

        pub fn is_blank(ctx: &mut EvalCtx, args: &[Expr]) -> Result<Value, Error> {
            match args {
                [] => Err(Error::InsufficientArgs),
                [arg] => Ok(matches!(eval(arg, ctx)?, Value::Bool(_)).into()),
                [_, ..] => Err(Error::TooManyArgs),
            }
        }

        pub fn is_formula(ctx: &mut EvalCtx, args: &[Expr]) -> Result<Value, Error> {
            match args {
                [] => Err(Error::InsufficientArgs),
                [arg] => Ok(matches!(eval(arg, ctx)?, Value::Formula(_)).into()),
                [_, ..] => Err(Error::TooManyArgs),
            }
        }
    }

    mod logical {
        use crate::{
            Error,
            cell::Value,
            language::{EvalCtx, Expr},
        };

        pub fn r#false(ctx: &mut EvalCtx, _: &[Expr]) -> Result<Value, Error> {
            Ok(false.into())
        }

        pub fn r#true(ctx: &mut EvalCtx, _: &[Expr]) -> Result<Value, Error> {
            Ok(true.into())
        }
    }

    mod math {
        use std::ops;

        use crate::{
            Error,
            cell::Value,
            language::{EvalCtx, Expr, eval},
        };

        pub fn abs(ctx: &mut EvalCtx, args: &[Expr]) -> Result<Value, Error> {
            match args {
                [] => Err(Error::InsufficientArgs),
                [arg] => {
                    let val = eval(arg, ctx)?;

                    todo!()
                }
                [_, ..] => Err(Error::TooManyArgs),
            }
        }

        pub fn power(ctx: &mut EvalCtx, args: &[Expr]) -> Result<Value, Error> {
            match args {
                [base, exponent] => {
                    let base = eval(base, ctx)?;
                    let exponent = eval(exponent, ctx)?;

                    match (base, exponent) {
                        (Value::Number(base), Value::Number(exponent)) => {
                            Ok(base.powf(exponent).into())
                        }
                        _ => Err(Error::RefMustBeNumber),
                    }
                }
                [_base, _exponent, ..] => Err(Error::TooManyArgs),
                _ => Err(Error::InsufficientArgs),
            }
        }

        pub fn sum(ctx: &mut EvalCtx, args: &[Expr]) -> Result<Value, Error> {
            let mut arg_vals = vec![];
            for arg in args {
                arg_vals.push(eval(arg, ctx)?);
            }
            arg_vals.into_iter().try_fold(Value::None, ops::Add::add)
        }
    }

    mod statistical {

        use crate::{
            Error,
            cell::Value,
            language::{EvalCtx, Expr, functions::sum},
        };

        pub fn average(ctx: &mut EvalCtx, args: &[Expr]) -> Result<Value, Error> {
            let len = args.len();
            if len == 0 {
                return Ok(Value::Number(0.0));
            }

            let sum = sum(ctx, args)?;

            sum / Value::Number(len as f64)
        }
    }
}

#[derive(Debug, Default, Clone, thiserror::Error, PartialEq)]
enum ParseError {
    #[error("Could not parse float")]
    FloatParse(#[from] ParseFloatError),
    #[default]
    #[error("Unexpected error")]
    Other,
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use chumsky::Parser;

    use crate::{
        Error, Spanleaf,
        cell::{CellIdx, Value},
        language::{EvalCtx, Expr, eval, parser},
        sheet::SheetIdx,
    };

    #[test]
    fn parsing() {
        let good_strings = [
            "1",
            " 1",
            "   1  ",
            "2+2",
            "3.14 / 2.02",
            "6.11e23",
            "9.1093837e-31",
            "-1",
            "-626.1",
            "-1234.5678e-9",
            "(2+2) - (6.1*2)",
            "sum(2, 3, 4)",
            "sum(2, 3, 4,)", // trailing comma let's go
            "[0, 0]",
            "[2, 3]",
            "4 * [2, 2+2]",
            "&[3, [2, 1]]",
            "sheet_name[0, 0]",
            "bad_sheet_name[1, 2]",
            "&sheet_name[6, 6]",
            "'words are words'",
        ];

        for s in good_strings {
            let parser = parser();
            let x = dbg!(parser.parse(s).unwrap());
        }
    }

    fn evaluate_dummy(expr: &Expr) -> Result<Value, Error> {
        eval(
            expr,
            &mut EvalCtx {
                sl: &mut Spanleaf::new(),
                curr_sheet: SheetIdx::next(),
                dependencies: &mut vec![],
            },
        )
    }

    #[test]
    fn evaluation() {
        let seven = Expr::Number(7.0);
        let five = Expr::Number(5.0);

        let sum = dbg!(Expr::Add(Box::new(seven.clone()), Box::new(five.clone())));

        let diff = dbg!(Expr::Sub(Box::new(seven.clone()), Box::new(five.clone())));

        let sum = dbg!(Expr::Add(Box::new(sum), Box::new(diff)));

        let x = dbg!(evaluate_dummy(&sum));
        dbg!((7.0 + 5.0) + (7.0 - 5.0));
    }

    #[test]
    fn function() {
        let sev = Expr::Number(7.0);

        let sum = Expr::Call("average".to_string(), vec![sev.clone(); 1000000]);

        let start = Instant::now();
        let res = dbg!(evaluate_dummy(&sum).unwrap());
        let end = Instant::now();

        println!("{:?}", end - start);
    }

    #[test]
    fn references() {
        let mut sl = Spanleaf::new();

        let s0 = sl.insert_sheet("sheet_name");
        let s1 = sl.insert_sheet("other_sheet");

        sl.insert(s0, CellIdx::new(0, 0), "=sum(1,2,3,4,)").unwrap();

        sl.insert(s1, CellIdx::new(1, 1), "=sheet_name[0,0]")
            .unwrap();

        dbg!(sl.get(s0, CellIdx::new(0, 0)).unwrap());
        dbg!(sl.get(s1, CellIdx::new(1, 1)).unwrap());

        // change source cell
        sl.insert(s0, CellIdx::new(0, 0), 12).unwrap();

        dbg!(sl.get(s0, CellIdx::new(0, 0)).unwrap());
        dbg!(sl.get(s1, CellIdx::new(1, 1)).unwrap());
    }
}
