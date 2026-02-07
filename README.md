# Spanleaf
An incremental rethinking of what a spreadsheet can be.

## Features:
- Custom formula language
- Row and Column default values
- Convenient relative offset system

## Motivation
I have long been a fan of spreadsheets. As a teenager, my dad gave me a copy of
"Intro to Microsoft Excel" he had been using and said "If you learn this, you'll
be a millionaire". Unfortunately, an adeptitude at spreadsheet manipulation hasn't
quite earned me that Ferrari yet, and being a millionaire isn't quite as luxurious
as it used to be. But my love of spreadsheets has continued through the cloud era,
and modern spreadsheet offerings have continuously given me an invaluable tool for
working through and reasoning about any number of problems, prototypes, and ideas.

Well, familiarity breeds contempt, and I am often frustrated by some limitations
that the common spreadsheets present, such as beginning every single new sheet by
filling the first column with the column number, needing to re-fill an entire column
any time I find an improvement to a formula, or needing a relative offset to the
given cell.

So I decided to create my own spreadsheet from the ground up, as is the obvious choice.

This is primarily a toy project and proof of concept, rolled into an experiment
with Rust for frontend, sprinkled with a dash of language design, and garnished
with just a kiss of data structures.
