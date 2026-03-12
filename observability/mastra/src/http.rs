use std::collections::BTreeMap;

use async_trait::async_trait;
use reqwest::Client;
use url::Url;

use crate::{ExportError, TraceBatch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    Post,
    Put,
    Patch,
}

impl HttpMethod {
    fn as_reqwest(self) -> reqwest::Method {
        match self {
            Self::Post => reqwest::Method::POST,
            Self::Put => reqwest::Method::PUT,
            Self::Patch => reqwest::Method::PATCH,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: Url,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

pub trait HttpRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError>;
}

#[async_trait]
pub trait ObservabilityExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError>;
}

#[derive(Clone, Debug)]
pub struct HttpExporter<B> {
    builder: B,
    client: Client,
}

impl<B> HttpExporter<B> {
    pub fn new(builder: B) -> Self {
        Self {
            builder,
            client: Client::new(),
        }
    }

    pub fn with_client(builder: B, client: Client) -> Self {
        Self { builder, client }
    }
}

impl<B> HttpExporter<B>
where
    B: HttpRequestBuilder,
{
    pub fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.builder.build_requests(batch)
    }
}

impl<B> HttpExporter<B>
where
    B: HttpRequestBuilder + Send + Sync,
{
    pub async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        for request in self.builder.build_requests(batch)? {
            let mut http_request = self
                .client
                .request(request.method.as_reqwest(), request.url.clone());

            for (key, value) in &request.headers {
                http_request = http_request.header(key, value);
            }

            let response = http_request.body(request.body.clone()).send().await?;
            let status = response.status();
            if !status.is_success() {
                return Err(ExportError::UnexpectedStatus {
                    url: request.url.clone(),
                    status_code: status.as_u16(),
                    response_body: response.text().await.unwrap_or_default(),
                });
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<B> ObservabilityExporter for HttpExporter<B>
where
    B: HttpRequestBuilder + Send + Sync,
{
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        HttpExporter::export(self, batch).await
    }
}
