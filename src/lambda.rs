use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use flate2::read::GzDecoder;
use hq::{process_html, HqConfig};
use lambda_http::{run, service_fn, Body, Error, Request, RequestExt, Response, tracing};
use serde_json::json;
use std::io::Read;

async fn fetch_html(
    url: &str,
    s3_client: &S3Client,
    offset: Option<u64>,
    length: Option<u64>,
) -> Result<String, Box<dyn std::error::Error>> {
    let data = if url.starts_with("s3://") {
        // Parse S3 URL: s3://bucket/key
        let s3_path = url.strip_prefix("s3://").ok_or("Invalid S3 URL")?;
        let parts: Vec<&str> = s3_path.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err("Invalid S3 URL format. Expected: s3://bucket/key".into());
        }
        let bucket = parts[0];
        let key = parts[1];

        let mut req = s3_client.get_object().bucket(bucket).key(key);

        if let (Some(offset), Some(length)) = (offset, length) {
            let range = format!("bytes={}-{}", offset, offset + length - 1);
            req = req.range(range);
        }

        let resp = req.send().await?;
        resp.body.collect().await?.to_vec()
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let client = reqwest::Client::new();
        let mut req = client.get(url);

        if let (Some(offset), Some(length)) = (offset, length) {
            let range = format!("bytes={}-{}", offset, offset + length - 1);
            req = req.header("Range", range);
        }

        let resp = req.send().await?;
        resp.bytes().await?.to_vec()
    } else {
        return Err("URL must start with http://, https://, or s3://".into());
    };

    // Check if content is gzipped and decompress
    let html = if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        // Gzip magic bytes detected
        let mut decoder = GzDecoder::new(&data[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed)?;
        decompressed
    } else {
        String::from_utf8(data)?
    };

    Ok(html)
}

async fn function_handler(event: Request, s3_client: &S3Client) -> Result<Response<Body>, Error> {
    let query_params = event.query_string_parameters();

    let url = query_params
        .first("url")
        .ok_or("Missing 'url' query parameter")?;

    let selector = query_params
        .first("selector")
        .unwrap_or(":root");

    let text_only = query_params
        .first("text")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let pretty = query_params
        .first("pretty")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let attributes: Vec<String> = query_params
        .all("attribute")
        .unwrap_or_default()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let offset = query_params
        .first("offset")
        .and_then(|s| s.parse::<u64>().ok());

    let length = query_params
        .first("length")
        .and_then(|s| s.parse::<u64>().ok());

    let compact = query_params
        .first("compact")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let config = HqConfig {
        selector: selector.to_string(),
        text_only,
        pretty_print: pretty,
        attributes,
        compact,
        ..Default::default()
    };

    match fetch_html(url, s3_client, offset, length).await {
        Ok(html) => match process_html(&html, &config) {
            Ok(result) => {
                let resp = Response::builder()
                    .status(200)
                    .header("content-type", "text/plain")
                    .body(result.into())
                    .map_err(Box::new)?;
                Ok(resp)
            }
            Err(e) => {
                let error_body = json!({
                    "error": "HTML processing failed",
                    "message": e.to_string()
                });
                let resp = Response::builder()
                    .status(400)
                    .header("content-type", "application/json")
                    .body(error_body.to_string().into())
                    .map_err(Box::new)?;
                Ok(resp)
            }
        },
        Err(e) => {
            let error_body = json!({
                "error": "Failed to fetch URL",
                "message": e.to_string()
            });
            let resp = Response::builder()
                .status(400)
                .header("content-type", "application/json")
                .body(error_body.to_string().into())
                .map_err(Box::new)?;
            Ok(resp)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = S3Client::new(&config);

    run(service_fn(|event: Request| async {
        function_handler(event, &s3_client).await
    }))
    .await
}
