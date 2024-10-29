//! This module will have all the requests and stuff related to talking with
//! [MyScript](https://www.myscript.com). Built based on their REST
//! documentation, seen [here](https://swaggerui.myscript.com).

use std::error::Error;
use std::path::Path;

use super::Stroke;

use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

/// Contains [Vec] of [Stroke]s.
#[derive(Default, Serialize)]
struct StrokeGroup {
    strokes: Vec<Stroke>,
}

/// Creates a [same-thread](tokio::runtime::Builder::new_current_thread)
/// tokio [runtime](tokio::runtime::Runtime) to run the HTTP request 
/// to the [MyScript API](https://swaggerui.myscript.com/#/Batch)
#[derive(Debug)]
pub struct MyScriptProcess {
    runtime: tokio::runtime::Runtime,
    command_hash: u64,
    // receiver: oneshot::Receiver<Result<String, reqwest::Error>>,
    handle: JoinHandle<Result<String, reqwest::Error>>,
}

impl Eq for MyScriptProcess {}

impl PartialEq for MyScriptProcess {
    fn eq(&self, other: &Self) -> bool {
        self.command_hash == other.command_hash
    }
}

/// Stores the keys needed to send requests to
/// the MyScript servers.
/// 
/// The API Keys will `default` to the ones seen in their
/// [GitHub](https://github.com/MyScript/iinkTS/blob/master/examples/server-configuration.json).
/// 
/// ### Use default values at your own risk.
#[derive(Deserialize, Clone)]
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
async fn transcribe(body: String, hmac: String, config: ServerConfig) -> Result<String, reqwest::Error> {
    use reqwest::Client;
    use reqwest::header::{ACCEPT, CONTENT_TYPE};

    Client::new()
        .post("https://cloud.myscript.com/api/v4.0/iink/batch")
        .header(ACCEPT, "application/json,application/vnd.myscript.jiix")
        .header("hmac", hmac)
        .header("applicationkey", config.api_key)
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .send().await?
        .text().await
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

impl MyScriptProcess {
    pub fn new(strokes: Vec<Stroke>, config: ServerConfig) -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build().unwrap();
        let body = build_body(strokes);
        let hmac = compute_hmac(&config, &body);
        let hash = crate::data_structures::hash(hmac.as_bytes());
        
        let handle = rt.spawn(transcribe(body, hmac, config));
        Self {
            runtime: rt,
            command_hash: hash,
            handle,
        }
    }

    /// Blocks the current thread until MyScript returns the
    /// response and returns that [response body](reqwest::Response::text)
    /// or any [errors](reqwest::Error)
    fn block_and_complete(self) -> Result<String, reqwest::Error> {
        self.runtime.block_on(async move {
            self.handle.await.unwrap()
        })
    }

    /// Blocks the current thread and returns the parsed response, with 
    /// any errors that may have occurred while sending the request or 
    /// parsing the response's JSON ([deserializing](Deserialize)).
    pub fn block_and_parse(self) -> Result<String, Box<dyn Error>> {
        let http_response = self.block_and_complete()?;
        let resp: MyScriptResponse = serde_json::from_str(&http_response)?;

        Ok(resp.into_string())
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
