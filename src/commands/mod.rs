use serde_json::{Value, json};

pub mod apm;
pub mod dashboards;
pub mod downtimes;
pub mod events;
pub mod hosts;
pub mod incidents;
pub mod logs;
pub mod metrics;
pub mod monitors;
pub mod notebooks;
pub mod rum;
pub mod services;
pub mod slos;
pub mod spans;
pub mod synthetics;
pub mod traces;

pub(crate) fn trace_id_query(trace_id: &str) -> String {
    let escaped = trace_id.replace('"', "\\\"");
    format!("@trace_id:\"{escaped}\"")
}

pub(crate) fn require_non_empty(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty."));
    }

    Ok(())
}

pub(crate) fn normalize_facet(field: &str) -> String {
    if field.starts_with('@') {
        field.to_string()
    } else {
        format!("@{field}")
    }
}

pub(crate) fn normalize_metric(metric: &str) -> String {
    if metric.starts_with('@') {
        metric.to_string()
    } else {
        format!("@{metric}")
    }
}

pub(crate) fn measure_sort(aggregation: &str, metric: Option<&str>) -> Value {
    let mut sort = json!({
        "type": "measure",
        "aggregation": aggregation,
        "order": "desc",
    });
    if let Some(metric) = metric {
        sort["metric"] = json!(metric);
    }
    sort
}

pub(crate) fn aggregate_compute(aggregation: &str, metric: Option<&str>) -> Value {
    let mut compute = json!({ "aggregation": aggregation });
    if let Some(metric) = metric {
        compute["metric"] = json!(metric);
    }
    compute
}

pub(crate) fn facet_group(field: &str, limit: u32, sort: Value) -> Value {
    json!({
        "facet": normalize_facet(field),
        "limit": limit,
        "sort": sort,
    })
}

pub(crate) fn search_request_body(
    query: &str,
    from: &str,
    to: &str,
    limit: u32,
    sort: Option<&str>,
    cursor: Option<&str>,
) -> Value {
    let mut page = json!({ "limit": limit });
    if let Some(cursor) = cursor {
        page["cursor"] = json!(cursor);
    }

    let mut attributes = json!({
        "filter": {
            "query": query,
            "from": from,
            "to": to,
        },
        "page": page,
    });
    if let Some(sort) = sort {
        attributes["sort"] = json!(sort);
    }

    json!({
        "data": {
            "type": "search_request",
            "attributes": attributes,
        }
    })
}

pub(crate) fn event_search_body(
    query: &str,
    from: &str,
    to: &str,
    limit: u32,
    cursor: Option<&str>,
) -> Value {
    let mut body = json!({
        "filter": {
            "query": query,
            "from": from,
            "to": to,
        },
        "page": { "limit": limit },
    });
    if let Some(cursor) = cursor {
        body["page"]["cursor"] = json!(cursor);
    }
    body
}

pub(crate) fn aggregate_request_body(
    query: &str,
    from: &str,
    to: &str,
    compute: Vec<Value>,
    group_by: Vec<Value>,
) -> Value {
    json!({
        "data": {
            "type": "aggregate_request",
            "attributes": {
                "filter": {
                    "query": query,
                    "from": from,
                    "to": to,
                },
                "compute": compute,
                "group_by": group_by,
            }
        }
    })
}
