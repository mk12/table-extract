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
//! let table = table_extract::Table::find_first(html).unwrap();
//! for row in &table {
//!     println!(
//!         "{} is {} years old",
//!         row.get("Name").unwrap_or("<name missing>"),
//!         row.get("Age").unwrap_or("<age missing>")
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
use std::collections::HashMap;

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
    /// specified in `headers`. The order does not matter.
    ///
    /// If `headers` is empty, this is the same as
    /// [`find_first`](#method.find_first).
    pub fn find_by_headers<T>(html: &str, headers: &[T]) -> Option<Self>
    where
        T: AsRef<str>,
    {
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
                    let cells = select_cells(tr, &sel_th);
                    headers.iter().all(|h| contains_str(&cells, h.as_ref()))
                })
            })
            .map(Self::new)
    }

    /// Returns the headers of the table.
    ///
    /// This will be empty if the table had no `<th>` tags in its first row. See
    /// [`Headers`](type.Headers.html) for more.
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Returns an iterator over the [`Row`](struct.Row.html)s of the table.
    ///
    /// Only `<td>` cells are considered when generating rows. If the first row
    /// of the table is a header row, meaning it contains at least one `<th>`
    /// cell, the iterator will start on the second row. Use
    /// [`headers`](#method.headers) to access the header row in that case.
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
#[derive(Debug, Eq, PartialEq)]
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

fn select_cells(element: ElementRef, selector: &Selector) -> Vec<String> {
    element.select(selector).map(|e| e.inner_html()).collect()
}

fn contains_str(slice: &[String], item: &str) -> bool {
    slice.iter().any(|s| s == item)
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
    fn test_find_by_headers_empty() {
        let headers: [&str; 0] = [];

        assert_eq!(None, Table::find_by_headers("", &headers));
        assert_eq!(None, Table::find_by_headers("foo", &headers));
        assert_eq!(None, Table::find_by_headers(HTML_NO_TABLE, &headers));

        assert!(Table::find_by_headers(TABLE_EMPTY, &headers).is_some());
        assert!(Table::find_by_headers(HTML_TWO_TABLES, &headers).is_some());
    }

    #[test]
    fn test_find_by_headers_none() {
        let headers = ["Name", "Age"];
        let bad_headers = ["Name", "BAD"];

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
        let headers: [&str; 0] = [];
        assert!(Table::find_by_headers(TABLE_TH, &headers).is_some());
        assert!(Table::find_by_headers(TABLE_TH_TD, &headers).is_some());
        assert!(Table::find_by_headers(HTML_TWO_TABLES, &headers).is_some());

        let headers = ["Name"];
        assert!(Table::find_by_headers(TABLE_TH, &headers).is_some());
        assert!(Table::find_by_headers(TABLE_TH_TD, &headers).is_some());
        assert!(Table::find_by_headers(HTML_TWO_TABLES, &headers).is_some());

        let headers = ["Age", "Name"];
        assert!(Table::find_by_headers(TABLE_TH, &headers).is_some());
        assert!(Table::find_by_headers(TABLE_TH_TD, &headers).is_some());
        assert!(Table::find_by_headers(HTML_TWO_TABLES, &headers).is_some());
    }

    #[test]
    fn test_headers_empty() {
        let empty = HashMap::new();
        assert_eq!(&empty, Table::find_first(TABLE_TD).unwrap().headers());
        assert_eq!(&empty, Table::find_first(TABLE_TD_TD).unwrap().headers());
    }

    #[test]
    fn test_headers_nonempty() {
        let mut headers = HashMap::new();
        headers.insert("Name".to_string(), 0);
        headers.insert("Age".to_string(), 1);

        assert_eq!(&headers, Table::find_first(TABLE_TH).unwrap().headers());
        assert_eq!(&headers, Table::find_first(TABLE_TH_TD).unwrap().headers());
        assert_eq!(&headers, Table::find_first(TABLE_TH_TH).unwrap().headers());

        headers.insert("Extra".to_string(), 2);
        assert_eq!(
            &headers,
            Table::find_first(TABLE_COMPLEX).unwrap().headers()
        );
    }

    #[test]
    fn test_iter_empty() {
        assert_eq!(0, Table::find_first(TABLE_EMPTY).unwrap().iter().count());
        assert_eq!(0, Table::find_first(TABLE_TH).unwrap().iter().count());
    }

    #[test]
    fn test_iter_nonempty() {
        assert_eq!(1, Table::find_first(TABLE_TD).unwrap().iter().count());
        assert_eq!(1, Table::find_first(TABLE_TH_TD).unwrap().iter().count());
        assert_eq!(2, Table::find_first(TABLE_TD_TD).unwrap().iter().count());
        assert_eq!(1, Table::find_first(TABLE_TH_TH).unwrap().iter().count());
        assert_eq!(4, Table::find_first(TABLE_COMPLEX).unwrap().iter().count());
    }

    #[test]
    fn test_row_is_empty() {
        let table = Table::find_first(TABLE_TD).unwrap();
        assert_eq!(
            vec![false],
            table.iter().map(|r| r.is_empty()).collect::<Vec<_>>()
        );

        let table = Table::find_first(TABLE_COMPLEX).unwrap();
        assert_eq!(
            vec![false, false, true, false],
            table.iter().map(|r| r.is_empty()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_row_len() {
        let table = Table::find_first(TABLE_TD).unwrap();
        assert_eq!(vec![2], table.iter().map(|r| r.len()).collect::<Vec<_>>());

        let table = Table::find_first(TABLE_COMPLEX).unwrap();
        assert_eq!(
            vec![2, 3, 0, 4],
            table.iter().map(|r| r.len()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_row_get_without_headers() {
        let table = Table::find_first(TABLE_TD).unwrap();
        let mut iter = table.iter();
        let row = iter.next().unwrap();

        assert_eq!(None, row.get(""));
        assert_eq!(None, row.get("foo"));
        assert_eq!(None, row.get("Name"));
        assert_eq!(None, row.get("Age"));

        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_row_get_with_headers() {
        let table = Table::find_first(TABLE_TH_TD).unwrap();
        let mut iter = table.iter();
        let row = iter.next().unwrap();

        assert_eq!(None, row.get(""));
        assert_eq!(None, row.get("foo"));
        assert_eq!(Some("John"), row.get("Name"));
        assert_eq!(Some("20"), row.get("Age"));

        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_row_get_complex() {
        let table = Table::find_first(TABLE_COMPLEX).unwrap();
        let mut iter = table.iter();

        let row = iter.next().unwrap();
        assert_eq!(Some("John"), row.get("Name"));
        assert_eq!(Some("20"), row.get("Age"));
        assert_eq!(None, row.get("Extra"));

        let row = iter.next().unwrap();
        assert_eq!(Some("May"), row.get("Name"));
        assert_eq!(Some("30"), row.get("Age"));
        assert_eq!(Some("foo"), row.get("Extra"));

        let row = iter.next().unwrap();
        assert_eq!(None, row.get("Name"));
        assert_eq!(None, row.get("Age"));
        assert_eq!(None, row.get("Extra"));

        let row = iter.next().unwrap();
        assert_eq!(Some("a"), row.get("Name"));
        assert_eq!(Some("b"), row.get("Age"));
        assert_eq!(Some("c"), row.get("Extra"));

        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_row_as_slice_without_headers() {
        let table = Table::find_first(TABLE_TD).unwrap();
        let mut iter = table.iter();

        assert_eq!(&["Name", "Age"], iter.next().unwrap().as_slice());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_row_as_slice_with_headers() {
        let table = Table::find_first(TABLE_TH_TD).unwrap();
        let mut iter = table.iter();

        assert_eq!(&["John", "20"], iter.next().unwrap().as_slice());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_row_as_slice_complex() {
        let table = Table::find_first(TABLE_COMPLEX).unwrap();
        let mut iter = table.iter();
        let empty: [&str; 0] = [];

        assert_eq!(&["John", "20"], iter.next().unwrap().as_slice());
        assert_eq!(&["May", "30", "foo"], iter.next().unwrap().as_slice());
        assert_eq!(&empty, iter.next().unwrap().as_slice());
        assert_eq!(&["a", "b", "c", "d"], iter.next().unwrap().as_slice());
        assert_eq!(None, iter.next());
    }
}
