//! Double-parse of the `remote-toolbox-sql` response.
//!
//! The tool returns `{"result":"[{\"COL\":val,...}]"}` where `result` is a
//! *stringified* JSON array and column names are UPPERCASE. Row structs should
//! therefore use `#[serde(rename_all = "UPPERCASE")]`.

use serde::de::DeserializeOwned;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("outer JSON did not match {{\"result\": string}}: {0}")]
    Outer(serde_json::Error),
    #[error("inner result JSON array did not parse: {0}")]
    Inner(serde_json::Error),
}

#[derive(Deserialize)]
struct Outer {
    result: String,
}

/// Parse the raw stdout of `remote-toolbox-sql` into typed rows.
///
/// `T` is the per-row struct; annotate it with `#[serde(rename_all = "UPPERCASE")]`
/// to map Oracle's uppercase column names.
pub fn parse_rows<T: DeserializeOwned>(raw: &str) -> Result<Vec<T>, ParseError> {
    let outer: Outer = serde_json::from_str(raw).map_err(ParseError::Outer)?;
    // The toolbox serializes an empty result set as the literal "null" (not "[]").
    if outer.result.trim() == "null" {
        return Ok(Vec::new());
    }
    serde_json::from_str(&outer.result).map_err(ParseError::Inner)
}

/// Parse into loosely-typed JSON values when the row shape is dynamic.
pub fn parse_values(raw: &str) -> Result<Vec<serde_json::Value>, ParseError> {
    parse_rows(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "UPPERCASE")]
    struct One {
        one: i64,
    }

    #[test]
    fn smoke_single_row() {
        // exactly the shape from the documented smoke test: {"result":"[{\"ONE\":1}]"}
        let raw = r#"{"result":"[{\"ONE\":1}]"}"#;
        let rows: Vec<One> = parse_rows(raw).unwrap();
        assert_eq!(rows, vec![One { one: 1 }]);
    }

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "UPPERCASE")]
    struct UtilRow {
        machno: String,
        k_util_capped: f64,
        logout_anomaly: i32,
    }

    #[test]
    fn typed_rows_with_floats_and_nulls() {
        let raw = r#"{"result":"[{\"MACHNO\":\"TT602\",\"K_UTIL_CAPPED\":0.969,\"LOGOUT_ANOMALY\":0},{\"MACHNO\":\"TT799\",\"K_UTIL_CAPPED\":0.951,\"LOGOUT_ANOMALY\":1}]"}"#;
        let rows: Vec<UtilRow> = parse_rows(raw).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].machno, "TT602");
        assert!((rows[0].k_util_capped - 0.969).abs() < 1e-9);
        assert_eq!(rows[1].logout_anomaly, 1);
    }

    #[test]
    fn empty_result_set() {
        let raw = r#"{"result":"[]"}"#;
        let rows: Vec<One> = parse_rows(raw).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn null_result_is_empty() {
        // the toolbox returns {"result":"null"} for a zero-row query
        let raw = r#"{"result":"null"}"#;
        let rows: Vec<One> = parse_rows(raw).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn optional_null_field() {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "UPPERCASE")]
        struct R {
            vessel: String,
            k_mph_gross: Option<f64>,
        }
        let raw = r#"{"result":"[{\"VESSEL\":\"SLSL\",\"K_MPH_GROSS\":null}]"}"#;
        let rows: Vec<R> = parse_rows(raw).unwrap();
        assert_eq!(rows[0].vessel, "SLSL");
        assert!(rows[0].k_mph_gross.is_none());
    }
}
