use anyhow::{Result, anyhow};
use magick_rust::{MagickWand, bindings};

//
// pub fn process_gps(wand: &MagickWand, drive: &Client) {
//     todo!();
// }

/*
#[cfg(test)]
mod tests {
    use std::borrow::Borrow;
    use google_drive2::{DriveHub, hyper, hyper_rustls, oauth2};
    use google_drive2::api::File;


    #[tokio::test]
    async fn new_client() {
        let key = oauth2::read_service_account_key("/Users/scryner/.kapy/cred.json").await.unwrap();

        let auth =
            oauth2::ServiceAccountAuthenticator::builder(key)
                .build().await.unwrap();

        let hub = DriveHub::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().enable_http2().build()), auth);

        let mut req = File::default();
        let result = hub.files().list().doit().await.unwrap();
        // let result = hub.apps().list().doit().await.unwrap();
        // let result = hub.drives().list().doit().await.unwrap();

        let file_list = result.1;

        for file in file_list.items.unwrap().iter() {
            let filename = file.original_filename.as_ref().unwrap();
            // let filename = file.name.as_ref().unwrap();
            println!("--- {}", filename);
        }
    }
}
 */