use std::sync::OnceLock;

use serde_json::{Map, Value};
use tabled::Table;
use tabled::settings::Style;

use crate::log;

#[derive(Clone, Debug, Default, clap::ValueEnum)]
pub enum Format {
    /// Curated columns per row, compact JSON
    #[default]
    Json,
    /// Human-readable table
    Table,
    /// Raw API response, compact JSON
    Full,
}

static MAX_TOKENS: OnceLock<usize> = OnceLock::new();

/// Token budget for projected JSON output. 0 disables it. Effective only on the first call.
pub fn set_max_tokens(budget: usize) {
    let _ = MAX_TOKENS.set(budget);
}

fn max_tokens() -> usize {
    MAX_TOKENS.get().copied().unwrap_or(0)
}

pub fn print_json(value: &Value) {
    println!("{}", serde_json::to_string(value).unwrap_or_default());
}

pub fn print_table(rows: &[Value], columns: &[&str]) {
    if rows.is_empty() {
        return;
    }

    let headers: Vec<String> = columns.iter().map(|c| c.to_string()).collect();
    let mut table_data: Vec<Vec<String>> = vec![headers];
    table_data.extend(resolve_cells(rows, columns));

    let table = Table::from_iter(table_data)
        .with(Style::rounded())
        .to_string();
    println!("{table}");
}

/// Resolve `columns` against each row, producing the formatted table cells.
pub fn resolve_cells(rows: &[Value], columns: &[&str]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| {
            columns
                .iter()
                .map(|col| format_cell(&resolve_path(row, col)))
                .collect()
        })
        .collect()
}

fn resolve_path(value: &Value, path: &str) -> Value {
    let mut current = value;
    for key in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map.get(key).unwrap_or(&Value::Null);
            }
            Value::Array(items) => {
                current = key
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| items.get(i))
                    .unwrap_or(&Value::Null);
            }
            _ => return Value::Null,
        }
    }
    current.clone()
}

fn last_segment(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// JSON key for a column: its last path segment, or the full path when another
/// column in the same list ends in the same segment.
fn column_key<'a>(columns: &[&'a str], column: &'a str) -> &'a str {
    let short = last_segment(column);
    let shared = columns.iter().filter(|c| last_segment(c) == short).count() > 1;
    if shared { column } else { short }
}

/// Build an object holding only `columns`, resolved against `row`. Unresolved columns are null.
pub fn project_row(row: &Value, columns: &[&str]) -> Value {
    let mut map = Map::new();
    for col in columns {
        map.insert(column_key(columns, col).to_string(), resolve_path(row, col));
    }
    Value::Object(map)
}

/// Compact JSON tokenizes at roughly three bytes per token.
fn estimate_tokens(value: &Value) -> usize {
    serde_json::to_string(value).map_or(0, |s| s.len().div_ceil(3))
}

/// Keep leading rows while their estimated tokens fit `budget` (0 = unlimited).
/// The first row is always kept. Returns the kept rows and the original count.
pub fn apply_budget(rows: Vec<Value>, budget: usize) -> (Vec<Value>, usize) {
    let total = rows.len();
    if budget == 0 {
        return (rows, total);
    }
    let mut used = 0;
    let mut kept = Vec::new();
    for row in rows {
        used += estimate_tokens(&row);
        if used > budget && !kept.is_empty() {
            break;
        }
        kept.push(row);
    }
    (kept, total)
}

/// Projected view of a list response: rows reduced to `columns`, under the
/// original wrapper key, with paging metadata preserved. Responses without a
/// row array are projected as a single object.
pub fn project(value: &Value, columns: &[&str], budget: usize) -> (Value, usize, usize) {
    let Some((wrapper, rows)) = find_rows(value) else {
        return (project_row(value, columns), 1, 1);
    };
    let projected: Vec<Value> = rows.iter().map(|r| project_row(r, columns)).collect();
    let (kept, total) = apply_budget(projected, budget);
    let shown = kept.len();
    let body = match wrapper {
        None => Value::Array(kept),
        Some(key) => {
            let mut map = Map::new();
            map.insert(key.to_string(), Value::Array(kept));
            for meta in ["meta", "metadata"] {
                if let Some(m) = value.get(meta) {
                    map.insert(meta.to_string(), m.clone());
                }
            }
            Value::Object(map)
        }
    };
    (body, shown, total)
}

fn print_projected(value: &Value, columns: &[&str]) -> usize {
    let budget = max_tokens();
    let (body, shown, total) = project(value, columns, budget);
    print_json(&body);
    if shown < total {
        log::warn(&format!(
            "Output truncated to {shown} of {total} rows by --max-tokens {budget}. Raise it or pass --max-tokens 0 to disable."
        ));
    }
    total
}

fn format_cell(value: &Value) -> String {
    match value {
        Value::Null => "-".into(),
        Value::String(s) => truncate(s, 80),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(a) => truncate(&format!("{} items", a.len()), 80),
        Value::Object(_) => truncate(&serde_json::to_string(value).unwrap_or_default(), 80),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max - 3).collect();
    format!("{truncated}...")
}

/// Print array output (data[], logs[], events[], etc.) and return row count.
/// `columns` are resolved relative to each row element. An empty `columns`
/// list prints the response as-is.
pub fn print_output(value: &Value, format: &Format, columns: &[&str]) -> usize {
    match format {
        Format::Json if !columns.is_empty() => print_projected(value, columns),
        Format::Json | Format::Full => {
            print_json(value);
            count_rows(value)
        }
        Format::Table => {
            let rows = extract_rows(value);
            let count = rows.len();
            print_table(&rows, columns);
            count
        }
    }
}

/// Print a single-object response (e.g., incidents get, notebooks get, apm primary-tags).
/// `columns` are resolved relative to the root response, rendered as a single table row.
pub fn print_object(value: &Value, format: &Format, columns: &[&str]) {
    match format {
        Format::Json if !columns.is_empty() => print_json(&project_row(value, columns)),
        Format::Json | Format::Full => print_json(value),
        Format::Table => print_table(std::slice::from_ref(value), columns),
    }
}

/// Count the number of result rows in a Datadog API response.
pub fn count_rows(value: &Value) -> usize {
    extract_rows(value).len()
}

const ROW_KEYS: &[&str] = &[
    "data",
    "logs",
    "events",
    "monitors",
    "host_list",
    "dashboards",
    "incidents",
    "notebooks",
    "series",
    "tests",
    "results",
    "slos",
];

/// Locate the row array of a Datadog response: the wrapper key it sits under
/// (None for a bare array) and the rows. Only arrays are unwrapped, not objects.
fn find_rows(value: &Value) -> Option<(Option<&'static str>, &Vec<Value>)> {
    for key in ROW_KEYS {
        if let Some(arr) = value.get(*key).and_then(Value::as_array) {
            return Some((Some(key), arr));
        }
    }
    value.as_array().map(|arr| (None, arr))
}

pub fn extract_rows(value: &Value) -> Vec<Value> {
    match find_rows(value) {
        Some((_, rows)) => rows.clone(),
        // Non-standard shapes: whole response as a single row
        None => vec![value.clone()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_path_simple() {
        let val = json!({"name": "test"});
        assert_eq!(resolve_path(&val, "name"), json!("test"));
    }

    #[test]
    fn test_resolve_path_nested() {
        let val = json!({"a": {"b": {"c": 42}}});
        assert_eq!(resolve_path(&val, "a.b.c"), json!(42));
    }

    #[test]
    fn test_resolve_path_missing() {
        let val = json!({"name": "test"});
        assert_eq!(resolve_path(&val, "missing"), Value::Null);
    }

    #[test]
    fn test_resolve_path_array_index() {
        let val = json!({"thresholds": [{"target": 99.9}, {"target": 95.0}]});
        assert_eq!(resolve_path(&val, "thresholds.1.target"), json!(95.0));
    }

    #[test]
    fn test_resolve_path_array_index_out_of_range() {
        let val = json!({"thresholds": [{"target": 99.9}]});
        assert_eq!(resolve_path(&val, "thresholds.5.target"), Value::Null);
    }

    #[test]
    fn test_resolve_path_array_nested_object() {
        let val = json!({"a": [{"b": "hit"}]});
        assert_eq!(resolve_path(&val, "a.0.b"), json!("hit"));
    }

    #[test]
    fn test_resolve_path_numeric_object_key() {
        let val = json!({"a": {"0": "by-key"}});
        assert_eq!(resolve_path(&val, "a.0"), json!("by-key"));
    }

    #[test]
    fn test_resolve_path_non_numeric_segment_on_array() {
        let val = json!({"a": [{"b": 1}]});
        assert_eq!(resolve_path(&val, "a.state"), Value::Null);
        assert_eq!(resolve_path(&val, "a.-1"), Value::Null);
    }

    #[test]
    fn test_resolve_cells_marks_unresolved_columns() {
        let rows = vec![json!({"id": 1, "status": "Alert"})];
        assert_eq!(
            resolve_cells(&rows, &["id", "status", "missing"]),
            vec![vec!["1", "Alert", "-"]]
        );
    }

    #[test]
    fn test_format_cell_null() {
        assert_eq!(format_cell(&Value::Null), "-");
    }

    #[test]
    fn test_format_cell_string() {
        assert_eq!(format_cell(&json!("hello")), "hello");
    }

    #[test]
    fn test_format_cell_number() {
        assert_eq!(format_cell(&json!(42)), "42");
    }

    #[test]
    fn test_format_cell_bool() {
        assert_eq!(format_cell(&json!(true)), "true");
    }

    #[test]
    fn test_format_cell_array() {
        assert_eq!(format_cell(&json!([1, 2, 3])), "3 items");
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("short", 80), "short");
    }

    #[test]
    fn test_truncate_long() {
        let long = "a".repeat(100);
        let result = truncate(&long, 80);
        assert_eq!(result.chars().count(), 80);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_utf8_safe() {
        // Multi-byte chars: each is 3 bytes. Should not panic.
        let s = "é".repeat(100);
        let result = truncate(&s, 20);
        assert_eq!(result.chars().count(), 20);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_extract_rows_data_array() {
        let val = json!({"data": [{"id": 1}, {"id": 2}]});
        assert_eq!(extract_rows(&val).len(), 2);
    }

    #[test]
    fn test_extract_rows_host_list() {
        let val = json!({"host_list": [{"name": "a"}, {"name": "b"}]});
        assert_eq!(extract_rows(&val).len(), 2);
    }

    #[test]
    fn test_extract_rows_single_object() {
        let val = json!({"type": "metric", "description": "cpu"});
        assert_eq!(extract_rows(&val).len(), 1);
    }

    #[test]
    fn test_count_rows_empty() {
        let val = json!({"data": []});
        assert_eq!(count_rows(&val), 0);
    }

    // ── Response-shape fixture tests ──────────────────────────────────

    #[test]
    fn test_extract_rows_spans_response() {
        // Realistic spans/events/search response shape
        let response = json!({
            "data": [
                {
                    "type": "spans",
                    "id": "span1",
                    "attributes": {
                        "service": "web",
                        "resource_name": "GET /api/users",
                        "duration": 1234567,
                        "status": "ok",
                        "trace_id": "abc123"
                    }
                },
                {
                    "type": "spans",
                    "id": "span2",
                    "attributes": {
                        "service": "db",
                        "resource_name": "SELECT users",
                        "duration": 567890,
                        "status": "ok",
                        "trace_id": "abc123"
                    }
                }
            ],
            "meta": { "page": { "after": "cursor123" } }
        });
        let rows = extract_rows(&response);
        assert_eq!(rows.len(), 2);
        // Verify column resolution works relative to row
        assert_eq!(resolve_path(&rows[0], "attributes.service"), json!("web"));
        assert_eq!(
            resolve_path(&rows[0], "attributes.resource_name"),
            json!("GET /api/users")
        );
        assert_eq!(resolve_path(&rows[1], "attributes.duration"), json!(567890));
    }

    #[test]
    fn test_extract_rows_events_response() {
        // Realistic events/search v2 response
        let response = json!({
            "data": [
                {
                    "type": "event",
                    "id": "evt1",
                    "attributes": {
                        "timestamp": "2026-03-17T10:00:00Z",
                        "message": "Deploy v1.2.3 started",
                        "attributes": {
                            "evt": { "name": "deploy.start" },
                            "status": "info"
                        }
                    }
                }
            ],
            "meta": {}
        });
        let rows = extract_rows(&response);
        assert_eq!(rows.len(), 1);
        // Verify the double-nested attributes path works
        assert_eq!(
            resolve_path(&rows[0], "attributes.attributes.evt.name"),
            json!("deploy.start")
        );
        assert_eq!(
            resolve_path(&rows[0], "attributes.timestamp"),
            json!("2026-03-17T10:00:00Z")
        );
        assert_eq!(
            resolve_path(&rows[0], "attributes.message"),
            json!("Deploy v1.2.3 started")
        );
    }

    #[test]
    fn test_single_object_data_not_unwrapped() {
        // Single-object data responses (e.g., metrics all-tags, incidents get)
        // should NOT be unwrapped by extract_rows
        let response = json!({
            "data": {
                "id": "trace.http.request.duration",
                "type": "metrics",
                "attributes": {
                    "tags": ["env", "service"]
                }
            }
        });
        // extract_rows should return the whole response as a single row (fallback)
        let rows = extract_rows(&response);
        assert_eq!(rows.len(), 1);
        // The row IS the whole response, so data.id resolves correctly
        assert_eq!(
            resolve_path(&rows[0], "data.id"),
            json!("trace.http.request.duration")
        );
    }

    #[test]
    fn test_dashboards_response() {
        let response = json!({
            "dashboards": [
                { "id": "abc-123", "title": "Production Overview", "layout_type": "ordered" },
                { "id": "def-456", "title": "API Metrics", "layout_type": "free" },
            ]
        });
        let rows = extract_rows(&response);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            resolve_path(&rows[0], "title"),
            json!("Production Overview")
        );
        assert_eq!(resolve_path(&rows[1], "id"), json!("def-456"));
    }

    #[test]
    fn test_host_list_response() {
        let response = json!({
            "host_list": [
                { "name": "web-1", "up": true, "meta": { "platform": "linux" } },
                { "name": "web-2", "up": false, "meta": { "platform": "linux" } },
            ],
            "total_matching": 2
        });
        let rows = extract_rows(&response);
        assert_eq!(rows.len(), 2);
        assert_eq!(resolve_path(&rows[0], "name"), json!("web-1"));
        assert_eq!(resolve_path(&rows[0], "meta.platform"), json!("linux"));
    }

    #[test]
    fn test_metrics_series_response() {
        let response = json!({
            "series": [
                { "metric": "system.cpu.user", "scope": "host:web-1", "pointlist": [[1710000000, 45.2]] },
            ]
        });
        let rows = extract_rows(&response);
        assert_eq!(rows.len(), 1);
        assert_eq!(resolve_path(&rows[0], "metric"), json!("system.cpu.user"));
    }

    #[test]
    fn test_format_cell_long_utf8_string() {
        // Ensure multi-byte characters don't panic in format_cell
        // Build a string > 80 chars using multi-byte characters
        let s = "日本語のテスト".repeat(20); // 7 * 20 = 140 chars
        let result = format_cell(&json!(s));
        assert!(result.chars().count() <= 80);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_print_output_returns_correct_count() {
        let response = json!({
            "data": [
                { "id": "1" },
                { "id": "2" },
                { "id": "3" },
            ]
        });
        let count = print_output(&response, &Format::Json, &["id"]);
        assert_eq!(count, 3);
    }

    // ── JSON projection ───────────────────────────────────────────────

    #[test]
    fn test_column_key_uses_last_segment() {
        let cols = &["attributes.service", "attributes.custom.duration"];
        assert_eq!(column_key(cols, "attributes.service"), "service");
        assert_eq!(column_key(cols, "attributes.custom.duration"), "duration");
    }

    #[test]
    fn test_column_key_falls_back_to_full_path_on_collision() {
        let cols = &["attributes.status", "attributes.attributes.status", "id"];
        assert_eq!(column_key(cols, "attributes.status"), "attributes.status");
        assert_eq!(
            column_key(cols, "attributes.attributes.status"),
            "attributes.attributes.status"
        );
        assert_eq!(column_key(cols, "id"), "id");
    }

    #[test]
    fn test_project_row_keeps_unresolved_as_null() {
        let row = json!({"id": 1, "attributes": {"service": "web"}});
        assert_eq!(
            project_row(&row, &["id", "attributes.service", "attributes.host"]),
            json!({"id": 1, "service": "web", "host": null})
        );
    }

    #[test]
    fn test_project_keeps_wrapper_and_meta_drops_the_rest() {
        let response = json!({
            "monitors": [
                {"id": 1, "name": "a", "tags": ["x"], "message": "long"},
                {"id": 2, "name": "b", "tags": ["y"], "message": "long"}
            ],
            "counts": {"status": [{"count": 2, "name": "Alert"}]},
            "metadata": {"page": 0, "total_count": 2}
        });
        let (body, shown, total) = project(&response, &["id", "name"], 0);
        assert_eq!(
            body,
            json!({
                "monitors": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}],
                "metadata": {"page": 0, "total_count": 2}
            })
        );
        assert_eq!((shown, total), (2, 2));
    }

    #[test]
    fn test_project_keeps_paging_cursor_in_meta() {
        let response = json!({
            "data": [{"id": "1", "attributes": {"service": "web", "tags": ["a", "b"]}}],
            "meta": {"page": {"after": "cursor"}}
        });
        let (body, _, _) = project(&response, &["attributes.service"], 0);
        assert_eq!(body["data"], json!([{"service": "web"}]));
        assert_eq!(body["meta"]["page"]["after"], json!("cursor"));
    }

    #[test]
    fn test_project_bare_array() {
        let response = json!([{"id": "1", "extra": true}, {"id": "2", "extra": false}]);
        let (body, _, total) = project(&response, &["id"], 0);
        assert_eq!(body, json!([{"id": "1"}, {"id": "2"}]));
        assert_eq!(total, 2);
    }

    #[test]
    fn test_project_object_without_rows_is_a_single_object() {
        let response = json!({"data": {"buckets": [1, 2]}, "meta": {"elapsed": 1}});
        let (body, shown, total) = project(&response, &["data.buckets"], 0);
        assert_eq!(body, json!({"buckets": [1, 2]}));
        assert_eq!((shown, total), (1, 1));
    }

    #[test]
    fn test_apply_budget_zero_is_unlimited() {
        let rows: Vec<Value> = (0..100).map(|i| json!({"i": i})).collect();
        let (kept, total) = apply_budget(rows, 0);
        assert_eq!((kept.len(), total), (100, 100));
    }

    #[test]
    fn test_apply_budget_cuts_rows_past_the_budget() {
        // Each row serializes to 7 bytes -> 3 estimated tokens
        let rows: Vec<Value> = (0..10).map(|i| json!({"i": i})).collect();
        let (kept, total) = apply_budget(rows, 7);
        assert_eq!(total, 10);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn test_apply_budget_always_keeps_first_row() {
        let rows = vec![
            json!({"message": "x".repeat(1000)}),
            json!({"message": "y"}),
        ];
        let (kept, total) = apply_budget(rows, 1);
        assert_eq!((kept.len(), total), (1, 2));
    }

    #[test]
    fn test_project_reports_truncation() {
        let response = json!({"data": (0..50).map(|i| json!({"id": i})).collect::<Vec<_>>()});
        let (body, shown, total) = project(&response, &["id"], 10);
        assert!(shown < total);
        assert_eq!(body["data"].as_array().unwrap().len(), shown);
    }
}
