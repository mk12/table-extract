// Copyright 2017 Mitchell Kember. Subject to the MIT License.

//! Utility for extracting data from HTML tables.
//!
//! This library allows you to parse tables from HTML documents and iterate over
//! their rows. There are three entry points:
//!
//! - [`Table::find_first`] finds the first table.
//! - [`Table::find_by_id`] finds a table by its HTML id.
//! - [`Table::find_by_headers`] finds a table that has certain headers.
//!
//! Each of these returns an `Option<`[`Table`]`>`, since there might not be any
//! matching table in the HTML. Once you have a table, you can iterate over it
//! and access the contents of each [`Row`].
//!
//! # Examples
//!
//! Here is a simple example that uses [`Table::find_first`] to print the fields
//! in each row of a table:
//!
//! ```
//! let html = r#"
//!     <table>
//!         <tr><th>Name</th><th>Age</th></tr>
//!         <tr><td>John</td><td>20</td></tr>
//!     </table>
//! "#;
//! let table = table_extract::Table::find_first(html);
//! for row in &table {
//!     println!(
//!         "{} is {} years old",
//!         row.get("Name").unwrap_or("<name missing>"),
//!         row.get("Age").unwrap_or_else("<age missing>")
//!     )
//! }
//! ```
//!
//! [`Table`]: struct.Table.html
//! [`Row`]: struct.Row.html
//! [`Table::find_first`]: struct.Table.html#method.find_first
//! [`Table::find_by_id`]: struct.Table.html#method.find_by_id
//! [`Table::find_by_headers`]: struct.Table.html#method.find_by_headers

extern crate scraper;

use scraper::element_ref::ElementRef;
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

/// A map from `<th>` table headers to their zero-based positions.
///
/// For example, consider the following table:
///
/// ```html
/// <table>
///     <tr><th>Name</th><th>Age</th></tr>
///     <tr><td>John</td><td>20</td></tr>
/// </table>
/// ```
///
/// The `Headers` for this table would map "Name" to 0 and "John" to 1.
pub type Headers = HashMap<String, usize>;

/// A parsed HTML table.
///
/// See [the module level documentation](index.html) for more.
#[derive(Debug, Eq, PartialEq)]
pub struct Table {
    headers: Headers,
    data: Vec<Vec<String>>,
}

impl Table {
    /// Finds the first table in `html`.
    pub fn find_first(html: &str) -> Option<Self> {
        let html = Html::parse_fragment(html);
        html.select(&css("table")).next().map(Self::new)
    }

    /// Finds the table in `html` with an id of `id`.
    pub fn find_by_id(html: &str, id: &str) -> Option<Self> {
        let html = Html::parse_fragment(html);
        let selector = format!("table#{}", id);
        Selector::parse(&selector)
            .ok()
            .as_ref()
            .map(|s| html.select(s))
            .and_then(|mut s| s.next())
            .map(Self::new)
    }

    /// Finds the table in `html` whose first row contains all of the headers
    /// specified in `headers`.
    ///
    /// If `headers` is empty, this is the same as
    /// [`find_first`](#method.find_first).
    pub fn find_by_headers(
        html: &str,
        headers: &HashSet<String>,
    ) -> Option<Self> {
        if headers.is_empty() {
            return Self::find_first(html);
        }

        let sel_table = css("table");
        let sel_tr = css("tr");
        let sel_th = css("th");

        let html = Html::parse_fragment(html);
        html.select(&sel_table)
            .find(|table| {
                table.select(&sel_tr).next().iter().any(|&tr| {
                    select_cells::<HashSet<_>>(tr, &sel_th).is_superset(headers)
                })
            })
            .map(Self::new)
    }

    /// Returns the headers of the table.
    ///
    /// This will be empty if the table had no `<th>` tags in its first row.
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Returns an iterator over the [`Row`](struct.Row.html)s of the table.
    pub fn iter(&self) -> Iter {
        Iter {
            headers: &self.headers,
            iter: self.data.iter(),
        }
    }

    fn new(element: ElementRef) -> Self {
        let sel_tr = css("tr");
        let sel_th = css("th");
        let sel_td = css("td");

        let mut headers = HashMap::new();
        let mut rows = element.select(&sel_tr).peekable();
        if let Some(tr) = rows.peek() {
            for (i, th) in tr.select(&sel_th).enumerate() {
                headers.insert(th.inner_html(), i);
            }
        }
        if !headers.is_empty() {
            rows.next();
        }
        let data = rows.map(|tr| select_cells(tr, &sel_td)).collect();

        Table { headers, data }
    }
}

impl<'a> IntoIterator for &'a Table {
    type Item = Row<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// An iterator over the rows in a [`Table`](struct.Table.html).
pub struct Iter<'a> {
    headers: &'a Headers,
    iter: std::slice::Iter<'a, Vec<String>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Row<'a>;

    fn next(&mut self) -> Option<Row<'a>> {
        let headers = self.headers;
        self.iter.next().map(|cells| Row { headers, cells })
    }
}

/// A row in a [`Table`](struct.Table.html).
///
/// A row consists of a number of data cells stored as strings. If the row
/// contains the same number of cells as the table's header row, its cells can
/// be safely accessed by header names using [`get`](#method.get). Otherwise,
/// the data should be accessed via [`as_slice`](#method.as_slice).
pub struct Row<'a> {
    headers: &'a Headers,
    cells: &'a [String],
}

impl<'a> Row<'a> {
    /// Returns the number of cells in the row.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns `true` if the row contains no cells.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Returns the cell underneath `header`.
    ///
    /// Returns `None` if there is no such header, or if there is no cell at
    /// that position in the row.
    pub fn get(&self, header: &str) -> Option<&'a str> {
        self.headers
            .get(header)
            .and_then(|&i| self.cells.get(i).map(String::as_str))
    }

    /// Returns a slice containing all the cells.
    pub fn as_slice(&self) -> &'a [String] {
        self.cells
    }
}

fn css(selector: &'static str) -> Selector {
    Selector::parse(selector).unwrap()
}

fn select_cells<T>(element: ElementRef, selector: &Selector) -> T
where
    T: FromIterator<String>,
{
    element.select(selector).map(|e| e.inner_html()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE_EMPTY: &'static str = r#"
<table></table>
"#;

    const TABLE_TH: &'static str = r#"
<table>
    <tr><th>Name</th><th>Age</th></tr>
</table>
"#;

    const TABLE_TD: &'static str = r#"
<table>
    <tr><td>Name</td><td>Age</td></tr>
</table>
"#;

    const TABLE_TH_TD: &'static str = r#"
<table>
    <tr><th>Name</th><th>Age</th></tr>
    <tr><td>John</td><td>20</td></tr>
</table>
"#;

    const TABLE_TD_TD: &'static str = r#"
<table>
    <tr><td>Name</td><td>Age</td></tr>
    <tr><td>John</td><td>20</td></tr>
</table>
"#;

    const TABLE_TH_TH: &'static str = r#"
<table>
    <tr><th>Name</th><th>Age</th></tr>
    <tr><th>John</th><th>20</th></tr>
</table>
"#;

    const TABLE_COMPLEX: &'static str = r#"
<table>
    <tr><th>Name</th><th>Age</th><th>Extra</th></tr>
    <tr><td>John</td><td>20</td></tr>
    <tr><td>May</td><td>30</td><td>foo</td></tr>
    <tr></tr>
    <tr><td>a</td><td>b</td><td>c</td><td>d</td></tr>
</table>
"#;

    const HTML_NO_TABLE: &'static str = r#"
<!doctype HTML>
<html>
    <head><title>foo</title></head>
    <body><p>Hi.</p></body>
</html>
"#;

    const HTML_TWO_TABLES: &'static str = r#"
<!doctype HTML>
<html>
    <head><title>foo</title></head>
    <body>
        <table id="first">
            <tr><th>Name</th><th>Age</th></tr>
            <tr><td>John</td><td>20</td></tr>
        </table>
        <table id="second">
            <tr><th>Name</th><th>Weight</th></tr>
            <tr><td>John</td><td>150</td></tr>
        </table>
    </body>
</html>
"#;


    #[test]
    fn test_find_first_none() {
        assert_eq!(None, Table::find_first(""));
        assert_eq!(None, Table::find_first("foo"));
        assert_eq!(None, Table::find_first(HTML_NO_TABLE));
    }

    #[test]
    fn test_find_first_empty() {
        let empty = Table {
            headers: HashMap::new(),
            data: Vec::new(),
        };
        assert_eq!(Some(empty), Table::find_first(TABLE_EMPTY));
    }

    #[test]
    fn test_find_first_some() {
        assert!(Table::find_first(TABLE_TH).is_some());
        assert!(Table::find_first(TABLE_TD).is_some());
    }

    #[test]
    fn test_find_by_id_none() {
        assert_eq!(None, Table::find_by_id("", ""));
        assert_eq!(None, Table::find_by_id("foo", "id"));
        assert_eq!(None, Table::find_by_id(HTML_NO_TABLE, "id"));

        assert_eq!(None, Table::find_by_id(TABLE_EMPTY, "id"));
        assert_eq!(None, Table::find_by_id(TABLE_TH, "id"));
        assert_eq!(None, Table::find_by_id(TABLE_TH, ""));
        assert_eq!(None, Table::find_by_id(HTML_TWO_TABLES, "id"));
    }

    #[test]
    fn test_find_by_id_some() {
        assert!(Table::find_by_id(HTML_TWO_TABLES, "first").is_some());
        assert!(Table::find_by_id(HTML_TWO_TABLES, "second").is_some());
    }

    #[test]
    fn test_find_by_headers_none() {
        let headers = hashset(vec!["Age", "Name"]);
        let bad_headers = hashset(vec!["Age", "BAD"]);

        assert_eq!(None, Table::find_by_headers("", &headers));
        assert_eq!(None, Table::find_by_headers("foo", &headers));
        assert_eq!(None, Table::find_by_headers(HTML_NO_TABLE, &headers));

        assert_eq!(None, Table::find_by_headers(TABLE_EMPTY, &bad_headers));
        assert_eq!(None, Table::find_by_headers(TABLE_TH, &bad_headers));

        assert_eq!(None, Table::find_by_headers(TABLE_TD, &headers));
        assert_eq!(None, Table::find_by_headers(TABLE_TD, &bad_headers));
    }

    #[test]
    fn test_find_by_headers_some() {
        let headers = HashSet::new();
        assert!(Table::find_by_headers(TABLE_TH, &headers).is_some());
        assert!(Table::find_by_headers(TABLE_TH_TD, &headers).is_some());
        assert!(Table::find_by_headers(HTML_TWO_TABLES, &headers).is_some());

        let headers = hashset(vec!["Name"]);
        assert!(Table::find_by_headers(TABLE_TH, &headers).is_some());
        assert!(Table::find_by_headers(TABLE_TH_TD, &headers).is_some());
        assert!(Table::find_by_headers(HTML_TWO_TABLES, &headers).is_some());

        let headers = hashset(vec!["Name", "Age"]);
        assert!(Table::find_by_headers(TABLE_TH, &headers).is_some());
        assert!(Table::find_by_headers(TABLE_TH_TD, &headers).is_some());
        assert!(Table::find_by_headers(HTML_TWO_TABLES, &headers).is_some());
    }

    #[test]
    fn test_iter() {}

    fn hashset(vec: Vec<&'static str>) -> HashSet<String> {
        HashSet::from_iter(vec.into_iter().map(String::from))
    }
}
