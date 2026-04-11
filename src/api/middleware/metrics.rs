use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use metrics::{counter, histogram, gauge};
use std::time::Instant;

/// Middleware to record HTTP request metrics for IAM service
pub async fn metrics_middleware(
    method: axum::http::Method,
    uri: axum::extract::OriginalUri,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let start = Instant::now();
    let path = uri.0.path().to_string();
    let method_str = method.to_string();

    // Record request in-flight
    gauge!("iam_http_requests_in_flight", "method" => method_str.clone(), "path" => path.clone()).increment(1.0);

    // Execute the request
    let response = next.run(request).await;
    let duration = start.elapsed();

    // Record metrics
    let status = response.status().as_u16();
    let status_str = status.to_string();

    // Total requests
    counter!("iam_http_requests_total", 
        "method" => method_str.clone(), 
        "path" => path.clone()
    ).increment(1);

    // Request duration histogram (in seconds)
    histogram!("iam_http_request_duration_seconds",
        "method" => method_str,
        "path" => path,
        "status" => status_str
    ).record(duration.as_secs_f64());

    // Error counter for 5xx
    if status >= 500 {
        counter!("iam_http_errors_total",
            "method" => method.to_string(),
            "path" => uri.0.path().to_string(),
            "status" => status.to_string()
        ).increment(1);
    }

    // Decrement in-flight counter
    gauge!("iam_http_requests_in_flight", "method" => method.to_string(), "path" => uri.0.path().to_string()).decrement(1.0);

    Ok(response)
}

/// Records authentication attempt metrics
pub fn record_auth_attempt(method: &str, success: bool) {
    counter!("iam_auth_attempts_total",
        "method" => method.to_string(),
        "success" => success.to_string()
    ).increment(1);

    if !success {
        counter!("iam_auth_failures_total",
            "method" => method.to_string()
        ).increment(1);
    }
}

/// Records authentication failure with reason
pub fn record_auth_failure(method: &str, reason: &str) {
    counter!("iam_auth_failures_total",
        "method" => method.to_string(),
        "reason" => reason.to_string()
    ).increment(1);
}

/// Records JWT operation metrics
pub fn record_jwt_operation(operation: &str, success: bool, duration_ms: f64) {
    counter!("iam_jwt_operations_total",
        "operation" => operation.to_string(),
        "success" => success.to_string()
    ).increment(1);

    histogram!("iam_jwt_operation_duration_ms",
        "operation" => operation.to_string()
    ).record(duration_ms);
}

/// Records API key operation metrics
pub fn record_api_key_operation(operation: &str, success: bool, duration_ms: f64) {
    counter!("iam_api_key_operations_total",
        "operation" => operation.to_string(),
        "success" => success.to_string()
    ).increment(1);

    histogram!("iam_api_key_operation_duration_ms",
        "operation" => operation.to_string()
    ).record(duration_ms);
}

/// Records user operation metrics
pub fn record_user_operation(operation: &str, success: bool) {
    counter!("iam_user_operations_total",
        "operation" => operation.to_string(),
        "success" => success.to_string()
    ).increment(1);
}

/// Records API key validation metrics
pub fn record_api_key_validation(success: bool, duration_ms: f64) {
    counter!("iam_api_key_validations_total",
        "success" => success.to_string()
    ).increment(1);

    histogram!("iam_api_key_validation_duration_ms").record(duration_ms);
}

/// Records Redis operation metrics
pub fn record_redis_operation(operation: &str, success: bool, duration_ms: f64) {
    counter!("iam_redis_operations_total",
        "operation" => operation.to_string(),
        "success" => success.to_string()
    ).increment(1);

    histogram!("iam_redis_operation_duration_ms",
        "operation" => operation.to_string()
    ).record(duration_ms);
}

/// Records active sessions count
pub fn record_active_sessions(count: u64) {
    metrics::gauge!("iam_active_sessions").set(count as f64);
}

/// Records gRPC request metrics for IAM service
pub async fn grpc_metrics_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let start = Instant::now();
    let path = req.uri().path().to_string();

    // Record request in-flight
    gauge!("iam_grpc_requests_in_flight", "method" => path.clone()).increment(1.0);

    // Execute the request
    let response = next.run(req).await;
    let duration = start.elapsed();

    // Record metrics
    let status = response.status().as_u16();
    let status_str = status.to_string();

    // Total requests
    counter!("iam_grpc_requests_total",
        "method" => path.clone(),
        "status" => status_str.clone()
    ).increment(1);

    // Request duration histogram (in seconds)
    histogram!("iam_grpc_request_duration_seconds",
        "method" => path.clone(),
        "status" => status_str
    ).record(duration.as_secs_f64());

    // Decrement in-flight counter
    gauge!("iam_grpc_requests_in_flight", "method" => path).decrement(1.0);

    Ok(response)
}
