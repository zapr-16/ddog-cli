//! Contract tests verifying the exact HTTP request shapes sent to Datadog APIs.
//! Uses wiremock to intercept and assert method, path, query params, and body.

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: build a DdClient that talks to the mock server.
fn mock_client(server: &MockServer) -> ddog::client::DdClient {
    ddog::client::DdClient::with_base_url(&server.uri())
}

// ── spans search ──────────────────────────────────────────────────────

#[tokio::test]
async fn spans_search_sends_data_attributes_envelope() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/events/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [],
            "meta": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::spans::SpansSearch {
        query: "service:web".into(),
        from: "1h".into(),
        to: None,
        limit: 25,
        sort: "-timestamp".into(),
        cursor: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::spans::search(&client, args).await;
    assert!(result.is_ok());
}

// ── traces get ────────────────────────────────────────────────────────

#[tokio::test]
async fn traces_get_sends_data_attributes_envelope() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/events/search"))
        .and(wiremock::matchers::body_partial_json(json!({
            "data": {
                "type": "search_request",
                "attributes": {
                    "filter": { "query": "@trace_id:\"abc123\"" },
                    "page": { "limit": 100 },
                    "sort": "timestamp"
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [],
            "meta": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::traces::TracesGet {
        trace_id: "abc123".into(),
        from: "1h".into(),
        to: None,
        limit: 100,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::traces::get(&client, args).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn spans_search_rejects_zero_limit_before_network() {
    let server = MockServer::start().await;

    let client = mock_client(&server);
    let args = ddog::commands::spans::SpansSearch {
        query: "*".into(),
        from: "1h".into(),
        to: None,
        limit: 0,
        sort: "-timestamp".into(),
        cursor: None,
        format: ddog::output::Format::Json,
    };

    let result = ddog::commands::spans::search(&client, args).await;
    let err = result.expect_err("expected limit=0 to fail validation");
    assert!(err.to_string().contains("Limit must be at least 1"));
}

// ── apm metrics (spans aggregate) ─────────────────────────────────────

#[tokio::test]
async fn apm_metrics_sends_aggregate_envelope() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/analytics/aggregate"))
        .and(wiremock::matchers::body_partial_json(json!({
            "data": {
                "type": "aggregate_request",
                "attributes": {
                    "compute": [{ "aggregation": "avg", "metric": "@duration" }],
                    "group_by": [{
                        "facet": "@service",
                        "limit": 25,
                        "sort": {
                            "type": "measure",
                            "aggregation": "avg",
                            "metric": "@duration",
                            "order": "desc"
                        }
                    }]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::apm::ApmCmd::Metrics {
        query: "service:web".into(),
        from: "1h".into(),
        to: None,
        compute: "avg".into(),
        metric: "duration".into(),
        group_by: vec!["service".into()],
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::apm::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn logs_analyze_group_by_uses_measure_sort() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/logs/analytics/aggregate"))
        .and(wiremock::matchers::body_partial_json(json!({
            "filter": { "query": "*" },
            "compute": [{
                "aggregation": "avg",
                "metric": "@duration"
            }],
            "group_by": [{
                "facet": "@service",
                "limit": 10,
                "sort": {
                    "type": "measure",
                    "aggregation": "avg",
                    "metric": "@duration",
                    "order": "desc"
                }
            }]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::logs::LogsCmd::Analyze {
        query: "*".into(),
        from: "1h".into(),
        to: None,
        compute: "avg".into(),
        metric: Some("duration".into()),
        group_by: vec!["service".into()],
        interval: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::logs::run(&client, cmd).await;
    assert!(result.is_ok());
}

// ── events search ─────────────────────────────────────────────────────

#[tokio::test]
async fn events_search_sends_correct_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/events/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [],
            "meta": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::events::EventsSearch {
        query: "source:deploy".into(),
        from: "1h".into(),
        to: None,
        limit: 25,
        cursor: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::events::search(&client, args).await;
    assert!(result.is_ok());
}

// ── metrics search (no query param) ──────────────────────────────────

#[tokio::test]
async fn metrics_search_sends_configured_filter() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v2/metrics"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::metrics::MetricsCmd::Search {
        tag: vec![],
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::metrics::run(&client, cmd).await;
    assert!(result.is_ok());
}

// ── monitors search ───────────────────────────────────────────────────

#[tokio::test]
async fn monitors_search_clamps_per_page_and_includes_sort() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/monitor/search"))
        .and(query_param("query", "status:alert"))
        .and(query_param("page", "2"))
        .and(query_param("per_page", "100"))
        .and(query_param("sort", "name"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "monitors": [
                {
                    "id": 1,
                    "name": "CPU high",
                    "type": "metric alert",
                    "overall_state": "Alert",
                    "query": "avg(last_5m):avg:system.cpu.user{*} > 90"
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::monitors::MonitorsSearch {
        query: "status:alert".into(),
        page: 2,
        per_page: 500,
        sort: Some("name".into()),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::monitors::search(&client, args).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn monitors_search_rejects_zero_per_page_before_network() {
    let server = MockServer::start().await;

    let client = mock_client(&server);
    let args = ddog::commands::monitors::MonitorsSearch {
        query: "".into(),
        page: 0,
        per_page: 0,
        sort: None,
        format: ddog::output::Format::Json,
    };
    let err = ddog::commands::monitors::search(&client, args)
        .await
        .expect_err("expected per_page=0 to fail validation");
    assert!(err.to_string().contains("Per page must be at least 1."));
}

// ── services search/deps ──────────────────────────────────────────────

#[tokio::test]
async fn services_search_sends_page_and_schema_params() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v2/services/definitions"))
        .and(query_param("page[size]", "50"))
        .and(query_param("page[number]", "3"))
        .and(query_param("schema_version", "v2.2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "attributes": {
                        "schema": {
                            "dd-service": "api",
                            "team": "platform",
                            "description": "API service"
                        }
                    }
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::services::ServicesCmd::Search {
        limit: 50,
        page: 3,
        schema: "v2.2".into(),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::services::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn services_deps_includes_primary_tag() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/service_dependencies"))
        .and(query_param("env", "production"))
        .and(query_param("primary_tag", "team:core"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "service_name": "web",
                "dependencies": ["db", "cache"]
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::services::ServicesCmd::Deps {
        env: "production".into(),
        primary_tag: Some("team:core".into()),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::services::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn services_search_rejects_zero_page_before_network() {
    let server = MockServer::start().await;

    let client = mock_client(&server);
    let cmd = ddog::commands::services::ServicesCmd::Search {
        limit: 20,
        page: 0,
        schema: "v2.1".into(),
        format: ddog::output::Format::Json,
    };
    let err = ddog::commands::services::run(&client, cmd)
        .await
        .expect_err("expected page=0 to fail validation");
    assert!(err.to_string().contains("Page must be at least 1."));
}

#[tokio::test]
async fn services_search_rejects_zero_limit_before_network() {
    let server = MockServer::start().await;

    let client = mock_client(&server);
    let cmd = ddog::commands::services::ServicesCmd::Search {
        limit: 0,
        page: 1,
        schema: "v2.1".into(),
        format: ddog::output::Format::Json,
    };
    let err = ddog::commands::services::run(&client, cmd)
        .await
        .expect_err("expected limit=0 to fail validation");
    assert!(err.to_string().contains("Limit must be at least 1."));
}

// ── hosts and dashboards validation ──────────────────────────────────

#[tokio::test]
async fn hosts_search_rejects_zero_count_before_network() {
    let server = MockServer::start().await;

    let client = mock_client(&server);
    let args = ddog::commands::hosts::HostsSearch {
        filter: "".into(),
        sort_field: None,
        sort_dir: None,
        start: 0,
        count: 0,
        include_muted: false,
        format: ddog::output::Format::Json,
    };
    let err = ddog::commands::hosts::search(&client, args)
        .await
        .expect_err("expected count=0 to fail validation");
    assert!(err.to_string().contains("Count must be at least 1."));
}

#[tokio::test]
async fn dashboards_search_rejects_zero_count_before_network() {
    let server = MockServer::start().await;

    let client = mock_client(&server);
    let args = ddog::commands::dashboards::DashboardsSearch {
        filter: None,
        count: 0,
        format: ddog::output::Format::Json,
    };
    let err = ddog::commands::dashboards::search(&client, args)
        .await
        .expect_err("expected count=0 to fail validation");
    assert!(err.to_string().contains("Count must be at least 1."));
}

// ── apm primary-tags (single-object response) ────────────────────────

#[tokio::test]
async fn apm_primary_tags_returns_ok() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v2/metrics/trace.http.request.duration/all-tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "id": "trace.http.request.duration",
                "type": "metrics",
                "attributes": {
                    "tags": ["env", "service", "host"]
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::apm::ApmCmd::PrimaryTags {
        metric: "trace.http.request.duration".into(),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::apm::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn apm_summary_quotes_numeric_trace_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/events/search"))
        .and(wiremock::matchers::body_partial_json(json!({
            "data": {
                "type": "search_request",
                "attributes": {
                    "filter": { "query": "@trace_id:\"6256131160983565787\"" },
                    "sort": "timestamp",
                    "page": { "limit": 1000 }
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::apm::ApmCmd::Summary {
        trace_id: "6256131160983565787".into(),
        from: "1h".into(),
        to: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::apm::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn traces_get_rejects_empty_trace_id_before_network() {
    let server = MockServer::start().await;

    let client = mock_client(&server);
    let args = ddog::commands::traces::TracesGet {
        trace_id: "".into(),
        from: "1h".into(),
        to: None,
        limit: 100,
        format: ddog::output::Format::Json,
    };
    let err = ddog::commands::traces::get(&client, args)
        .await
        .expect_err("expected empty trace_id to fail validation");
    assert!(err.to_string().contains("Trace ID must not be empty."));
}

// ── spans search verifies envelope body structure ─────────────────────

#[tokio::test]
async fn spans_search_body_has_data_type_attributes() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/events/search"))
        .and(wiremock::matchers::body_partial_json(json!({
            "data": {
                "type": "search_request",
                "attributes": {
                    "filter": { "query": "service:api" },
                    "sort": "-timestamp",
                    "page": { "limit": 10 },
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [],
            "meta": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::spans::SpansSearch {
        query: "service:api".into(),
        from: "1h".into(),
        to: None,
        limit: 10,
        sort: "-timestamp".into(),
        cursor: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::spans::search(&client, args).await;
    assert!(result.is_ok());
}

// ── apm aggregate verifies envelope body structure ────────────────────

#[tokio::test]
async fn apm_aggregate_body_has_data_type_attributes() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/analytics/aggregate"))
        .and(wiremock::matchers::body_partial_json(json!({
            "data": {
                "type": "aggregate_request",
                "attributes": {
                    "filter": { "query": "*" },
                    "compute": [{ "aggregation": "pc99", "metric": "@duration" }],
                    "group_by": [{
                        "facet": "@resource_name",
                        "limit": 25,
                        "sort": {
                            "type": "measure",
                            "aggregation": "pc99",
                            "metric": "@duration",
                            "order": "desc"
                        }
                    }]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::apm::ApmCmd::Bottlenecks {
        query: "*".into(),
        from: "1h".into(),
        to: None,
        group_by: "resource_name".into(),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::apm::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn apm_tags_body_uses_measure_sort_for_count() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/analytics/aggregate"))
        .and(wiremock::matchers::body_partial_json(json!({
            "data": {
                "type": "aggregate_request",
                "attributes": {
                    "compute": [{ "aggregation": "count" }],
                    "group_by": [{
                        "facet": "@service",
                        "limit": 100,
                        "sort": {
                            "type": "measure",
                            "aggregation": "count",
                            "order": "desc"
                        }
                    }]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::apm::ApmCmd::Tags {
        query: "*".into(),
        from: "1h".into(),
        to: None,
        facet: "service".into(),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::apm::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn apm_latency_tags_body_uses_measure_sort_for_avg() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/analytics/aggregate"))
        .and(wiremock::matchers::body_partial_json(json!({
            "data": {
                "type": "aggregate_request",
                "attributes": {
                    "compute": [
                        { "aggregation": "avg", "metric": "@duration" },
                        { "aggregation": "pc95", "metric": "@duration" },
                        { "aggregation": "count" }
                    ],
                    "group_by": [{
                        "facet": "@region",
                        "limit": 50,
                        "sort": {
                            "type": "measure",
                            "aggregation": "avg",
                            "metric": "@duration",
                            "order": "desc"
                        }
                    }]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::apm::ApmCmd::LatencyTags {
        query: "*".into(),
        from: "1h".into(),
        to: None,
        tag: "region".into(),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::apm::run(&client, cmd).await;
    assert!(result.is_ok());
}

// ── dashboards search ─────────────────────────────────────────────────

#[tokio::test]
async fn dashboards_search_no_filter() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/dashboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "dashboards": [
                { "id": "abc", "title": "Production Overview" },
                { "id": "def", "title": "Staging Metrics" },
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::dashboards::DashboardsSearch {
        filter: None,
        count: 100,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::dashboards::search(&client, args).await;
    assert!(result.is_ok());
}

// ── dashboards filter pages through all results ───────────────────────

#[tokio::test]
async fn dashboards_filter_pages_through_results() {
    let server = MockServer::start().await;

    // First page: 100 dashboards, none matching
    let mut page1 = Vec::new();
    for i in 0..100 {
        page1.push(json!({ "id": format!("d{i}"), "title": format!("Other Dashboard {i}") }));
    }
    // Second page: has the match
    let page2 = vec![
        json!({ "id": "match1", "title": "Production API" }),
        json!({ "id": "d101", "title": "Another Dashboard" }),
    ];

    Mock::given(method("GET"))
        .and(path("/api/v1/dashboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "dashboards": page1
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/dashboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "dashboards": page2
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::dashboards::DashboardsSearch {
        filter: Some("Production".into()),
        count: 100,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::dashboards::search(&client, args).await;
    assert!(result.is_ok());
}

// ── incidents get/search ──────────────────────────────────────────────

#[tokio::test]
async fn incidents_get_includes_related_resources() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v2/incidents/abc123"))
        .and(query_param("include", "users,attachments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "id": "abc123",
                "attributes": {
                    "title": "API outage",
                    "severity": "SEV-1",
                    "state": "active",
                    "created": "2026-03-17T00:00:00Z"
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::incidents::IncidentsCmd::Get {
        id: "abc123".into(),
        include: Some("users,attachments".into()),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::incidents::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn incidents_search_clamps_limit_to_100() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v2/incidents/search"))
        .and(query_param("query", "state:active"))
        .and(query_param("sort", "-created"))
        .and(query_param("page[size]", "100"))
        .and(query_param("page[offset]", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": "incident-1",
                    "attributes": {
                        "title": "API outage",
                        "severity": "SEV-1",
                        "state": "active"
                    }
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::incidents::IncidentsCmd::Search {
        query: "state:active".into(),
        sort: "-created".into(),
        limit: 500,
        offset: 20,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::incidents::run(&client, cmd).await;
    assert!(result.is_ok());
}

// ── notebooks get/search ──────────────────────────────────────────────

#[tokio::test]
async fn notebooks_get_returns_ok() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/notebooks/12345"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "id": 12345,
                "attributes": {
                    "name": "Postmortem",
                    "author": { "handle": "user@example.com" },
                    "modified": "2026-03-17T00:00:00Z"
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::notebooks::NotebooksCmd::Get {
        id: 12345,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::notebooks::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn notebooks_search_includes_author_filter() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/notebooks"))
        .and(query_param("query", "deploy"))
        .and(query_param("author_handle", "user@example.com"))
        .and(query_param("sort_field", "name"))
        .and(query_param("sort_dir", "asc"))
        .and(query_param("start", "10"))
        .and(query_param("count", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": 12345,
                    "attributes": {
                        "name": "Deployment Notes",
                        "author": { "handle": "user@example.com" },
                        "modified": "2026-03-17T00:00:00Z"
                    }
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::notebooks::NotebooksCmd::Search {
        query: "deploy".into(),
        author: Some("user@example.com".into()),
        sort_field: "name".into(),
        sort_dir: "asc".into(),
        start: 10,
        count: 25,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::notebooks::run(&client, cmd).await;
    assert!(result.is_ok());
}

// ── rum search ────────────────────────────────────────────────────────

#[tokio::test]
async fn rum_search_sends_data_attributes_envelope() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/rum/events/search"))
        .and(wiremock::matchers::body_partial_json(json!({
            "data": {
                "type": "search_request",
                "attributes": {
                    "filter": { "query": "@type:action" },
                    "sort": "-timestamp",
                    "page": { "limit": 25 }
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [],
            "meta": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::rum::RumSearch {
        query: "@type:action".into(),
        from: "1h".into(),
        to: None,
        limit: 25,
        sort: "-timestamp".into(),
        cursor: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::rum::search(&client, args).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn rum_search_rejects_zero_limit_before_network() {
    let server = MockServer::start().await;

    let client = mock_client(&server);
    let args = ddog::commands::rum::RumSearch {
        query: "*".into(),
        from: "1h".into(),
        to: None,
        limit: 0,
        sort: "-timestamp".into(),
        cursor: None,
        format: ddog::output::Format::Json,
    };

    let result = ddog::commands::rum::search(&client, args).await;
    let err = result.expect_err("expected limit=0 to fail validation");
    assert!(err.to_string().contains("Limit must be at least 1"));
}

// ── slos search/get/history ───────────────────────────────────────────

#[tokio::test]
async fn slos_search_sends_query_params() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/slo"))
        .and(query_param("query", "service:web"))
        .and(query_param("limit", "50"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": "slo-abc",
                    "name": "Web Availability",
                    "type": "metric",
                    "overall_status": [{ "state": "OK" }],
                    "thresholds": [{ "target": 99.9 }]
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::slos::SlosCmd::Search {
        ids: None,
        query: "service:web".into(),
        limit: 50,
        offset: 0,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::slos::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn slos_get_returns_ok() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/slo/slo-abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "id": "slo-abc123",
                "name": "API Availability",
                "type": "metric",
                "thresholds": [{ "target": 99.95 }]
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::slos::SlosCmd::Get {
        id: "slo-abc123".into(),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::slos::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn slos_history_sends_epoch_params() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/slo/slo-abc123/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "overall": {
                    "sli_value": 99.95,
                    "span_precision": 1.0,
                    "name": "API Availability"
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::slos::SlosCmd::History {
        id: "slo-abc123".into(),
        from: "7d".into(),
        to: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::slos::run(&client, cmd).await;
    assert!(result.is_ok());
}

// ── downtimes list/get ────────────────────────────────────────────────

#[tokio::test]
async fn downtimes_list_sends_query_params() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v2/downtime"))
        .and(query_param("current_only", "true"))
        .and(query_param("page[limit]", "25"))
        .and(query_param("page[offset]", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": "dt-abc",
                    "attributes": {
                        "display_name": "Deploy window",
                        "status": "active",
                        "scope": "env:prod",
                        "schedule": { "start": "2026-03-27T00:00:00Z" }
                    }
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::downtimes::DowntimesCmd::List {
        current_only: true,
        limit: 25,
        offset: 0,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::downtimes::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn downtimes_get_returns_ok() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v2/downtime/dt-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "id": "dt-abc",
                "attributes": {
                    "display_name": "Deploy window",
                    "status": "active",
                    "scope": "env:prod",
                    "schedule": { "start": "2026-03-27T00:00:00Z" }
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::downtimes::DowntimesCmd::Get {
        id: "dt-abc".into(),
        include: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::downtimes::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn downtimes_get_with_include() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v2/downtime/dt-xyz"))
        .and(query_param("include", "created_by,monitor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "id": "dt-xyz",
                "attributes": {
                    "display_name": "Maintenance",
                    "status": "scheduled"
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::downtimes::DowntimesCmd::Get {
        id: "dt-xyz".into(),
        include: Some("created_by,monitor".into()),
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::downtimes::run(&client, cmd).await;
    assert!(result.is_ok());
}

// ── synthetics list/results ───────────────────────────────────────────

#[tokio::test]
async fn synthetics_list_sends_page_params() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/synthetics/tests"))
        .and(query_param("page_size", "25"))
        .and(query_param("page_number", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tests": [
                {
                    "public_id": "abc-xyz-123",
                    "name": "Homepage Check",
                    "type": "api",
                    "status": "live",
                    "locations": ["aws:us-east-1"]
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::synthetics::SyntheticsCmd::List {
        page_size: 25,
        page_number: 0,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::synthetics::run(&client, cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn synthetics_results_sends_time_params() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/synthetics/tests/abc-xyz-123/results"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {
                    "result_id": "r-1",
                    "status": 0,
                    "check_time": 1711500000,
                    "dc_id": 1
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let cmd = ddog::commands::synthetics::SyntheticsCmd::Results {
        id: "abc-xyz-123".into(),
        from: "6h".into(),
        to: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::synthetics::run(&client, cmd).await;
    assert!(result.is_ok());
}

// ── error handling: 403 ───────────────────────────────────────────────

#[tokio::test]
async fn api_403_returns_permission_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/hosts"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::hosts::HostsSearch {
        filter: "".into(),
        sort_field: None,
        sort_dir: None,
        start: 0,
        count: 10,
        include_muted: false,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::hosts::search(&client, args).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("403"));
    assert!(err.contains("Permission denied"));
}

// ── error handling: 401 ───────────────────────────────────────────────

#[tokio::test]
async fn api_401_returns_auth_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/spans/events/search"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let args = ddog::commands::spans::SpansSearch {
        query: "*".into(),
        from: "1h".into(),
        to: None,
        limit: 10,
        sort: "-timestamp".into(),
        cursor: None,
        format: ddog::output::Format::Json,
    };
    let result = ddog::commands::spans::search(&client, args).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("401"));
    assert!(err.contains("invalid or expired"));
}
