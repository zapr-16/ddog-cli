use serde_json::Value;
use tabled::Table;
use tabled::settings::Style;

#[derive(Clone, Debug, Default, clap::ValueEnum)]
pub enum Format {
    #[default]
    Json,
    Table,
}

pub fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

pub fn print_table(rows: &[Value], columns: &[&str]) {
    if rows.is_empty() {
        return;
    }

    let headers: Vec<String> = columns.iter().map(|c| c.to_string()).collect();
    let mut table_data: Vec<Vec<String>> = vec![headers];

    for row in rows {
        let mut cells = Vec::new();
        for col in columns {
            let val = resolve_path(row, col);
            cells.push(format_cell(&val));
        }
        table_data.push(cells);
    }

    let table = Table::from_iter(table_data)
        .with(Style::rounded())
        .to_string();
    println!("{table}");
}

fn resolve_path(value: &Value, path: &str) -> Value {
    let mut current = value;
    for key in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map.get(key).unwrap_or(&Value::Null);
            }
            _ => return Value::Null,
        }
    }
    current.clone()
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
/// `columns` are resolved relative to each row element.
pub fn print_output(value: &Value, format: &Format, columns: &[&str]) -> usize {
    match format {
        Format::Json => {
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
        Format::Json => print_json(value),
        Format::Table => print_table(std::slice::from_ref(value), columns),
    }
}

/// Count the number of result rows in a Datadog API response.
pub fn count_rows(value: &Value) -> usize {
    extract_rows(value).len()
}

fn extract_rows(value: &Value) -> Vec<Value> {
    // Try common Datadog response shapes — only unwrap arrays, not objects
    for key in &[
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
    ] {
        if let Some(inner) = value.get(*key)
            && let Some(arr) = inner.as_array()
        {
            return arr.clone();
        }
    }
    if let Some(arr) = value.as_array() {
        return arr.clone();
    }
    // Fallback: whole response as single row (for non-standard shapes)
    vec![value.clone()]
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
}
