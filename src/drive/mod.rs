pub mod auth;
mod helper;

use std::collections::HashMap;
use anyhow::{anyhow, Result};
use bytes::Bytes;
use serde::Deserialize;
use auth::GoogleAuthenticator;
use url::Url;
use reqwest;
use reqwest::{header, StatusCode};
use reqwest::blocking::Response;

pub struct GoogleDrive {
    authenticator: GoogleAuthenticator,
}

const GOOGLE_DRIVE_API_V3_FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";

impl GoogleDrive {
    pub fn new(authenticator: GoogleAuthenticator) -> Self {
        GoogleDrive { authenticator }
    }

    pub fn list(&self, q: &str, page_size: usize, next_page_token: Option<&str>) -> Result<ListResponse> {
        // make params
        let mut params = HashMap::new();
        params.insert("q", String::from(q));

        let page_size_str = format!("{}", page_size);
        params.insert("pageSize", page_size_str);

        if let Some(page_token) = next_page_token {
            params.insert("pageToken", String::from(page_token));
        }

        // request
        let u = format!("{}", GOOGLE_DRIVE_API_V3_FILES_URL);
        let res = self.request(u, params)?;

        if res.status() == StatusCode::OK {
            Ok(res.json::<ListResponse>()?)
        } else {
            Err(anyhow!("Failed to request: {}", res.status()))
        }
    }

    #[allow(dead_code)]
    pub fn get(&self, file_id: &str) -> Result<GetResponse> {
        let u = format!("{}/{}", GOOGLE_DRIVE_API_V3_FILES_URL, file_id);
        let res = self.request(u, HashMap::new())?;

        if res.status() == StatusCode::OK {
            Ok(res.json::<GetResponse>()?)
        } else {
            Err(anyhow!("Failed to request: {}", res.status()))
        }
    }

    pub fn download_blob(&self, file_id: &str) -> Result<Bytes> {
        let u = format!("{}/{}?alt=media", GOOGLE_DRIVE_API_V3_FILES_URL, file_id);
        let res = self.request(u, HashMap::new())?;

        if res.status() == StatusCode::OK {
            Ok(res.bytes()?)
        } else {
            Err(anyhow!("Failed to request: {}", res.status()))
        }
    }

    fn request(&self, u: String, params: HashMap<&str, String>) -> Result<Response> {
        // get access token
        let access_token = self.authenticator.access_token()?;

        // make url
        let u = Url::parse_with_params(&u, params.iter())?;

        // request
        let cli = reqwest::blocking::Client::new();
        let res = cli.get(u)
            .bearer_auth(access_token.secret())
            .header(header::ACCEPT, "application/json")
            .send()?;

        if res.status() == StatusCode::OK {
            Ok(res)
        } else {
            Err(anyhow!("Failed to request: {}", res.status()))
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ListResponse {
    pub kind: String,
    pub incomplete_search: bool,
    pub files: Vec<FileMetadata>,
    pub next_page_token: Option<String>,
}

#[allow(dead_code)]
pub type GetResponse = FileMetadata;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct FileMetadata {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub mime_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use chrono::Utc;
    use url::form_urlencoded;
    use crate::drive::auth::{CredPath, ListenPort};

    #[test]
    fn build_url_param() {
        let params = HashMap::from([
            ("q", "name contains 'gpx'"),
            ("pageSize", "100"),
        ]);

        let q = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params)
            .finish();

        println!("q = {}", q);
    }


    #[test]
    fn parse_result_json() {
        let result_json = r#"{
  "kind": "drive#fileList",
  "incompleteSearch": false,
  "files": [
    {
      "kind": "drive#file",
      "mimeType": "application/gpx+xml",
      "id": "10WZrfXHdDLU5M5SC_MZdiqaXFZs-b1j0",
      "name": "2023-02-03 - 2023_02_03.gpx"
    },
    {
      "kind": "drive#file",
      "mimeType": "application/gpx+xml",
      "id": "1lNuJCNkXjrUkJIDF6gKlVztQkvNPrnx-",
      "name": "2023-01-30 - 2023_01_30.gpx"
    }
  ]
}"#;

        let list_resp: ListResponse = serde_json::from_str(result_json).unwrap();
        print!("{:#?}", list_resp);
    }

    #[test]
    fn list_google_drive() {
        let auth = GoogleAuthenticator::new(ListenPort::DefaultPort, CredPath::DefaultPath);
        let drive = GoogleDrive::new(auth);

        let created_at = Utc::now();
        let created_at = created_at.format("%Y-%m-%dT%H:%M:%S");

        let q = format!("createdTime < '{}' and mimeType='application/gpx+xml'", created_at);
        println!("query = {}", q);

        let list_resp = drive.list(&q, 10, None).unwrap();

        println!("{:#?}", list_resp);
    }

    #[test]
    fn download_blob_from_google_drive() {
        let auth = GoogleAuthenticator::new(ListenPort::DefaultPort, CredPath::DefaultPath);
        let drive = GoogleDrive::new(auth);

        let file_id = "1lNuJCNkXjrUkJIDF6gKlVztQkvNPrnx-";
        let dn_resp = drive.download_blob(file_id).unwrap();
        let  v = dn_resp.to_vec();
        let s = String::from_utf8(v).unwrap();

        println!("{}", s);
    }
}