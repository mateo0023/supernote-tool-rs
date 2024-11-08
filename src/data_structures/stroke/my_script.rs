//! This module will have all the requests and stuff related to talking with
//! [MyScript](https://www.myscript.com). Built based on their REST
//! documentation, seen [here](https://swaggerui.myscript.com).

use std::sync::Arc;
use std::{error::Error, fmt::Display};
use std::path::Path;

use super::Stroke;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum TransciptionError {
    Server(reqwest::Error),
    Response(serde_json::Error),
}

/// Contains [Vec] of [Stroke]s.
#[derive(Default, Serialize)]
struct StrokeGroup {
    strokes: Vec<Stroke>,
}

/// Stores the keys needed to send requests to
/// the MyScript servers.
/// 
/// The API Keys will `default` to the ones seen in their
/// [GitHub](https://github.com/MyScript/iinkTS/blob/master/examples/server-configuration.json).
/// 
/// ### Use default values at your own risk.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    #[serde(rename = "applicationKey")]
    api_key: String,
    #[serde(rename = "hmacKey")]
    hmac_key: String,
}

/// The struct that contains the relevant information
/// from the response.
/// 
/// The response contains many other attributes that
/// are not needed.
#[derive(Deserialize)]
struct MyScriptResponse {
    /// The actual transcribed text.
    label: String,
}

/// Will transcribe the given set of
/// [StrokeGroup](https://swaggerui.myscript.com/#/Batch%20mode/batch#StrokeGroup)s
pub async fn transcribe(strokes: Vec<Stroke>, config: Arc<RwLock<ServerConfig>>) -> Result<String, TransciptionError> {
    use reqwest::Client;
    use reqwest::header::{ACCEPT, CONTENT_TYPE};
    
    let config = config.read().await;

    let body = build_body(strokes);
    let hmac = compute_hmac(&config, &body);

    let http_response = Client::new()
        .post("https://cloud.myscript.com/api/v4.0/iink/batch")
        .header(ACCEPT, "application/json,application/vnd.myscript.jiix")
        .header("hmac", hmac)
        .header("applicationkey", &config.api_key)
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .send().await?.text().await?;
    
    let resp: MyScriptResponse = serde_json::from_str(&http_response)?;

    Ok(resp.into_string())
}

/// Computes the HMAC given the [ServerConfig] and
/// body (`data`) of the request. See the
/// [example](https://developer.myscript.com/support/account/registering-myscript-cloud/#computing-the-hmac-value)
fn compute_hmac(config: &ServerConfig, data: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha512;
    let start = format!("{}{}", config.api_key, config.hmac_key);
    let mut mac = Hmac::<Sha512>::new_from_slice(start.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(data.as_bytes());

    let res = mac.finalize();

    hex::encode(res.into_bytes())
}

/// Builds the body of the request as a JSON.
/// This includes the **configuration** for the response.
/// 
/// **See** [REST API](https://swaggerui.myscript.com/#/)
/// and [Jiix Docs](https://developer.myscript.com/docs/interactive-ink/3.2/reference/configuration/)
/// 
/// Uses the [serde_json::json!] macro.
fn build_body(strokes: Vec<Stroke>) -> String {
    serde_json::json!({
        "contentType": "Text",
        "configuration": {
            "export": {
                "jiix": {
                    "bounding-box": false,
                    "strokes": false,
                    "ids": false,
                    "full-stroke-ids": false,
                    "text": {
                        "chars": false,
                        "words": true
                    }
                }
            },
            "lang": "en_US",
            "text": {
                "guides": {
                    "enable": true
                },
                "eraser": {
                    "erase-precisely": false
                },
                "mimeTypes": [
                    "application/vnd.myscript.jiix"
                ]
            }
        },
        "strokeGroups": [{
            "strokes": serde_json::to_value(strokes).unwrap(),
        }]
    }).to_string()
}

impl Display for TransciptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransciptionError::Server(error) => write!(f, "{}", error),
            TransciptionError::Response(error) => write!(f, "{}", error),
        }
    }
}

impl Error for TransciptionError {}

impl From<reqwest::Error> for TransciptionError {
    fn from(value: reqwest::Error) -> Self {
        Self::Server(value)
    }
}
impl From<serde_json::Error> for TransciptionError {
    fn from(value: serde_json::Error) -> Self {
        Self::Response(value)
    }
}

impl From<&[Stroke]> for StrokeGroup {
    /// Convert to the [StrokeGroup] while also shifting
    /// from time_deltas to standard time.
    fn from(value: &[Stroke]) -> Self {
        let mut timer = 0;
        Self {
            strokes: value.iter().map(|s| {
                let mut exp = s.clone();
                for time in exp.time.iter_mut() {
                    timer += *time;
                    *time = timer;
                }
                timer += 100;
                exp
            }).collect()
        }
    }
}

impl ServerConfig {
    /// Loads the [API Keys](ServerConfig) from the given `path`.
    pub fn from_path<P: AsRef<Path>> (path: P) -> Result<Self, Box<dyn Error>> {
        use std::fs::File;
        Ok(serde_json::from_reader(File::open(path)?)?)
    }

    /// See [Self::from_path()].
    #[inline]
    pub fn from_path_or_default<P: AsRef<Path>> (path: P) -> Self {
        Self::from_path(path).unwrap_or_default()
    }
}

impl Default for ServerConfig {
    /// Default constructor.
    /// # USE AT OWN RISK
    /// This returns the Keys used for the MyScript
    /// [Examples](https://github.com/MyScript/iinkTS/blob/master/examples/server-configuration.json).
    /// It is your responsibility to ensure you're not
    /// breaking any terms and conditions
    fn default() -> Self {
        Self {
            api_key: "58cce6d2-d2a7-4ad3-b3bf-166f7b43619e".to_string(),
            hmac_key: "92731ec6-605b-4a07-8b82-076675cd25ed".to_string(),
        }
    }
}

impl MyScriptResponse {
    fn into_string(self) -> String {
        self.label.replace('\n', " ")
    }
}
