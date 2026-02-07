use dioxus::prelude::*;
use spanleaf_core::{
    cell::{CellIdx, Value},
    sheet::{SheetIdx, ValueResult, ValueSource},
    Spanleaf,
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
// const HEADER_SVG: Asset = asset!("/assets/header.svg");

fn main() {
    info!("Start");

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    info!("Starting");

    let mut sl = Spanleaf::new();
    let sref = sl.insert_sheet("Sheet1");

    {
        // row and col defaults
        sl.insert_row_default(sref, 7, 1).unwrap();
        sl.insert_col_default(sref, 4, "apple").unwrap();
        // use the current row or column as a value
        sl.insert_col_default(sref, 0, "=r").unwrap();

        // explicit values overwrite row and col defaults
        sl.insert(sref, CellIdx::new(0, 0), -3).unwrap();

        // reference other cells
        sl.insert(sref, CellIdx::new(2, 4), "=[0, 0]").unwrap();

        // fibonacci
        sl.insert(sref, CellIdx::new(0, 10), 0).unwrap();
        sl.insert(sref, CellIdx::new(1, 10), 1).unwrap();
        // functions in formulae
        sl.insert_col_default(sref, 10, "=sum([r-1, c], [r-2, c])")
            .unwrap();

        // golden ratio approx
        sl.insert_col_default(sref, 11, "=[r-1, c-1] / [r, c-1]")
            .unwrap();

        // setting explicit references
        for i in 1..10 {
            sl.insert(sref, CellIdx::new(0, i), format!("=[0, {}] + 1", i - 1))
                .unwrap();
            sl.insert(sref, CellIdx::new(i, i), format!("={i} * {i}"))
                .unwrap();
        }

        sl.insert(sref, CellIdx::new(11, 2), "Lorem Ipsum").unwrap();
        // reference indirection
        sl.insert(sref, CellIdx::new(12, 2), "=&[11, 2]").unwrap();
        // value is now a reference
        sl.insert(sref, CellIdx::new(13, 2), "=[12, 2]").unwrap();
        // dereference the indirect cell reference
        sl.insert(sref, CellIdx::new(14, 2), "=*[13, 2]").unwrap();

        // cyclic dependency error
        sl.insert(sref, CellIdx::new(12, 6), "=[12, 7]").unwrap();
        sl.insert(sref, CellIdx::new(12, 7), "=[12, 6]").unwrap();
    }

    let sl = use_signal(move || sl);
    let curr_sheet = use_signal(move || sref);
    let curr_elem = use_signal(|| ActiveElement::Cell(CellIdx::new(0, 0)));

    info!("Creating sheet");

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        FormulaBar { sl, curr_sheet, curr_elem }
        Cells { sl, curr_sheet, curr_elem }
        Sheets { sl, curr_sheet }
    }
}

#[derive(Clone, Copy)]
pub enum ActiveElement {
    Row(u64),
    Col(u64),
    Cell(CellIdx),
}

#[component]
pub fn FormulaBar(
    sl: Signal<Spanleaf>,
    curr_sheet: Signal<SheetIdx>,
    curr_elem: Signal<ActiveElement>,
) -> Element {
    let sref = curr_sheet();
    let active_el = curr_elem();

    let (raw_value, curr) = {
        let sl = sl.read();
        match active_el {
            ActiveElement::Row(row) => {
                (sl.get_row_default(sref, row).value(), format!("Row {row}"))
            }
            ActiveElement::Col(col) => {
                (sl.get_col_default(sref, col).value(), format!("Col {col}"))
            }
            ActiveElement::Cell(cref) => {
                let val = sl.get_raw_value(sref, cref);
                (
                    match val.source {
                        ValueSource::Native => val.value(),
                        ValueSource::RowDefault | ValueSource::ColDefault => Value::None,
                    },
                    format!("[{}, {}]", cref.row, cref.col),
                )
            }
        }
    };

    rsx! {
        div { class: "formula-bar",
            div { class: "current-cell-display", "{curr}" }

            input {
                id: "formula-entry",
                onchange: move |evt| {
                    evt.prevent_default();

                    info!("{evt:?}");
                    match active_el {
                        ActiveElement::Row(row) => {
                            sl.write().insert_row_default(sref, row, evt.value()).unwrap();
                        }
                        ActiveElement::Col(col) => {
                            sl.write().insert_col_default(sref, col, evt.value()).unwrap();
                        }
                        ActiveElement::Cell(cref) => {
                            sl.write().insert(sref, cref, evt.value()).unwrap();
                        }
                    };
                    info!("Updated");
                },
                value: "{raw_value}",
            }

        }
    }
}

#[component]
pub fn Cells(
    sl: Signal<Spanleaf>,
    curr_sheet: Signal<SheetIdx>,
    curr_elem: Signal<ActiveElement>,
) -> Element {
    info!("Rendering cells");

    let sref = curr_sheet.read();
    let display_rows = 30;
    let display_cols = 30;

    let (row_defaults, col_defaults) = {
        let sl = sl.read();
        (
            (0..display_rows)
                .map(|row| (row, sl.get_row_default(*sref, row)))
                // .map(|i| (i, ()))
                .collect::<Vec<_>>(),
            (0..display_cols)
                .map(|col| (col, sl.get_col_default(*sref, col)))
                // .map(|i| (i, ()))
                .collect::<Vec<_>>(),
        )
    };

    info!("finished getting defaults");

    rsx! {
        div { class: "cells-container",
            table { class: "cells",
                tr {
                    // empty corner
                    th { "" }

                    // header row
                    for (col , val) in col_defaults {
                        HeaderCell { idx: col, val, curr_elem }
                    }
                }

                for (row , default_val) in row_defaults {
                    tr {
                        // header col
                        HeaderCell { idx: row, val: default_val, curr_elem }

                        for col in 0..display_cols {

                            Cell {
                                sl,
                                sref: *sref,
                                cref: CellIdx { row, col },
                                curr_elem,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn HeaderCell(idx: u64, val: ValueResult, curr_elem: Signal<ActiveElement>) -> Element {
    let mut class = "cell".to_string();

    let s = match val.source {
        ValueSource::Native => String::new(),
        ValueSource::RowDefault => {
            {
                if let ActiveElement::Row(row) = curr_elem() {
                    if row == idx {
                        class.push_str(" active-elem");
                    }
                }
            }

            if let Value::None = val.value {
                format!("[{idx}]")
            } else {
                class.push_str(" row-default");
                format!("[{idx}] {}", val.value)
            }
        }
        ValueSource::ColDefault => {
            if let ActiveElement::Col(col) = curr_elem() {
                if col == idx {
                    class.push_str(" active-elem");
                }
            }

            if let Value::None = val.value {
                format!("[{idx}]")
            } else {
                class.push_str(" col-default");
                format!("[{idx}] {}", val.value)
            }
        }
    };

    rsx! {
        th {
            class,
            onclick: move |_| {
                match val.source {
                    ValueSource::Native => todo!(),
                    ValueSource::RowDefault => *curr_elem.write() = ActiveElement::Row(idx),
                    ValueSource::ColDefault => *curr_elem.write() = ActiveElement::Col(idx),
                }
            },
            "{s}"
        }
    }
}

#[component]
pub fn Cell(
    sl: Signal<Spanleaf>,
    sref: SheetIdx,
    cref: CellIdx,
    curr_elem: Signal<ActiveElement>,
) -> Element {
    let raw = sl.read().get_raw_value(sref, cref).value();
    let val = sl.read().get(sref, cref);
    let mut class = "cell".to_string();

    let (s, title) = match val {
        Ok(val) => {
            match &val.source {
                ValueSource::Native => {}
                ValueSource::RowDefault => class.push_str(" row-default"),
                ValueSource::ColDefault => class.push_str(" col-default"),
            };
            (val.to_string(), raw.to_string())
        }
        Err(e) => {
            class.push_str(" error-cell");
            ("#ERROR".to_string(), format!("{e:?}"))
        }
    };

    {
        if let ActiveElement::Cell(cell) = curr_elem() {
            if cell == cref {
                class.push_str(" active-elem");
            }
        }
    }

    rsx! {
        td {
            class,
            title,
            onclick: move |_| {
                *curr_elem.write() = ActiveElement::Cell(cref);
            },
            "{s}"
        }
    }
}

#[component]
pub fn Sheets(sl: Signal<Spanleaf>, curr_sheet: Signal<SheetIdx>) -> Element {
    rsx! {
        div { class: "sheet-footer" }
    }
}
