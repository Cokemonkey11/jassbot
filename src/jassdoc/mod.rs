use oops::Oops;
use serde::Deserialize;
use urlencoding::encode;

#[derive(Deserialize)]
pub struct QueryParsed {
    pub contents: String,
    pub tag: String,
}

#[derive(Deserialize)]
pub struct NativeResponse {
    #[serde(rename = "queryParsed")]
    pub query_parsed: QueryParsed,
    pub results: Vec<String>,
}

#[derive(Deserialize)]
pub struct Annotation {
    pub name: String,
    pub value: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct Parameter {
    pub doc: Option<String>,
    pub name: String,

    #[serde(rename = "type")]
    pub type_: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct DocResponse {
    pub annotations: Vec<Annotation>,
    pub commit: String,
    pub kind: String,
    pub linenumber: String,
    pub parameters: Vec<Parameter>,
}

pub async fn jassdoc_doc_response_of(query: &str) -> std::io::Result<DocResponse> {
    let json_str = reqwest::get(format!(
        "https://lep.duckdns.org/app/jassbot/doc/api/{}",
        encode(query)
    ))
    .await
    .oops("Request failed")?;

    serde_json::from_str::<DocResponse>(&json_str.text().await.oops("Failed to get body")?)
        .oops("Failed to deserialize response")
}

pub async fn jassdoc_native_response_of(query: &str) -> std::io::Result<NativeResponse> {
    let json_str = reqwest::get(format!(
        "https://lep.duckdns.org/app/jassbot/search/api/{}",
        encode(query)
    ))
    .await
    .oops("Request failed")?;

    serde_json::from_str::<NativeResponse>(&json_str.text().await.oops("Failed to get body")?)
        .oops("Failed to deserialize response")
}

pub fn jassdoc_user_doc_uri_of(query: &str) -> String {
    format!("https://lep.duckdns.org/jassbot/doc/{}", encode(query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_native() -> std::io::Result<()> {
        let resp = jassdoc_native_response_of("CreateUnit").await?;

        assert_eq!(resp.results.into_iter().next().oops("test failed")?, "native CreateUnit takes player id, integer unitid, real x, real y, real face returns unit");

        Ok(())
    }

    #[tokio::test]
    async fn test_api_doc() -> std::io::Result<()> {
        let resp = jassdoc_doc_response_of("CreateUnit").await?;

        assert_eq!(
            resp.parameters.into_iter().next().oops("test failed")?.name,
            "id"
        );

        Ok(())
    }
}
