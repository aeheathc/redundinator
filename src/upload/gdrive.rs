extern crate hyper;
extern crate hyper_rustls;
extern crate rustls;

use chrono::{DateTime, Duration, Utc};
use google_drive3::{api::{File, Scope}, Delegate, DriveHub, hyper_util, yup_oauth2 as oauth2};
use google_apis_common::{MethodInfo, Retry};
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_rustls::HttpsConnector;
use log::{error, /*warn,*/ info, /*debug,*/ trace, /*log, Level*/};
use std::{fs, path::PathBuf};

type Hub = DriveHub<HttpsConnector<HttpConnector>>;

use crate::backoff::calculate_backoff_series;
use crate::settings::app_settings::Settings;
use crate::{new_tokio_runtime, upload::list_files};

/**
Upload exports to Google Drive for a given source.

If an auth token doesn't exist, this will give instructions on stdout and wait indefinitely to
recieve the signal from Google that you've followed them. In other words, it will go
interactive. However, this should only happen once as token refreshes are handled automatically.

# Arguments
* `source_name` - Name of the source for which to upload files.
* `settings` - The whole settings object for the app.
* `parent` - Destination folder on Google Drive. Note that this is the folder ID, not the name. Get this by calling get_parent.

# Returns
bool for whether uploading was found to be possible. The actual uploads may or may not have succeeded, but if this is false no more uploads should be attempted.
*/
pub fn gdrive_up(source_name: &str, settings: &Settings) -> bool
{
    info!("Starting Google Drive upload of exports for source: {source_name}");
    let runtime = match new_tokio_runtime()
    {
        Ok(r) => r,
        Err(e) => {error!("Couldn't create tokio runtime! Error: {e}"); return false;}
    };
    runtime.block_on(
        upload_files(source_name, settings)
    )
}

/**
Get the Hub object on which you can call all the google drive interaction functions.
As needed, this will handle the token cache file, authenticate to Google, and build the Hub.

# Arguments
* `settings` - The whole settings object for the app.

# Returns
The hub object, or None if something failed
*/
async fn connect(settings: &Settings) -> Option<Hub>
{
    trace!("Opening connection to Google Drive");

    //make sure cache dir exists then generate path to token cache file
    let mut cache_path = PathBuf::from(&settings.startup.cache_dir);
    if let Err(e) = fs::create_dir_all(&cache_path)
    {
        error!("Couldn't create directory to cache google drive tokens. Dir: {} -- Error: {e}", cache_path.to_string_lossy());
        return None;
    }
    cache_path.push("gdrive_tokens.json");

    // Authenticate to Google
    let service_account_key = match oauth2::read_service_account_key(&settings.gdrive.service_account_key_file).await {
        Ok(k) => k,
        Err(e) => {error!("Couldn't read gdrive key file: {e}"); return None;}
    };
    let connector = match hyper_rustls::HttpsConnectorBuilder::new().with_native_roots() {Ok(b)=>b,Err(e)=>{error!("Couldn't set hyper tls settings: {e}"); return None;}}
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();
    let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(connector);
    let auth = match oauth2::ServiceAccountAuthenticator::builder(service_account_key).persist_tokens_to_disk(cache_path).subject(&settings.gdrive.email).build().await
    {
        Ok(a) => a,
        Err(e) => {error!("Couldn't authenticate to Google: {e}"); return None;}
    };
    let hub = DriveHub::new(client, auth);
    Some(hub)
}

async fn check_free_space(hub: &Hub, upload_size: u64) -> bool
{
    let upload_size: i64 = match upload_size.try_into()
    {
        Ok(u) => u,
        Err(e) => {
            error!("File too big for Google Drive API, which uses i64 for file sizes (stopping uploads): {e}");
            return false;
        }
    };

    let (_, result) = match hub.about().get().param("fields", "*").doit().await
    {
        Ok(a) => a,
        Err(e) => {
            error!("Couldn't check free space on google drive (stopping uploads): {e}");
            return false;
        }
    };
    if let Some(max_upload_size) = result.max_upload_size
    {
        if max_upload_size < upload_size
        {
            error!("File to upload ({upload_size}) bigger than max_upload_size ({max_upload_size}), stopping uploads");
            return false;
        }
    }
    if let Some(about) = result.storage_quota
    {
        if let Some(limit) = about.limit
        {
            if let Some(usage) = about.usage
            {
                let drive_usage = about.usage_in_drive.unwrap_or(0);
                let remaining = limit - usage;
                if remaining < upload_size
                {
                    error!("File to upload ({upload_size}) bigger than free space remaining, stopping uploads. Limit: {limit} Usage: {usage} total, ({drive_usage} of which is in Drive). Remaining: {remaining}");
                    return false;
                }
            }else{
                if limit < upload_size
                {
                    error!("File to upload ({upload_size}) bigger than storage limit ({limit}), stopping uploads");
                    return false;
                }
            }
        }
    }

    true
}

/**
Upload all the files for a given source.

Don't upload files that are already there on the google drive. That way it can be ran repeatedly
in case of error until all the files are up.

# Returns
bool for whether uploading was found to be possible. The actual uploads may or may not have succeeded, but if this is false no more uploads should be attempted.
*/
async fn upload_files(source_name: &str, settings: &Settings) -> bool
{
    trace!("Enumerating exports for source: {source_name}");
    let hub = match connect(settings).await {Some(h)=>h,None=>{return false;}};
    let mime_str = "application/octet-stream"; //"application/octet-stream";
    let mime_type: mime::Mime = match mime_str.parse() {Ok(f)=>f,Err(e)=>{error!("Couldn't parse mime type! Error: {e}");return false;}};
    let parent = settings.gdrive.dir_id.clone();
    
    // Prepare upload
    for filename in list_files(source_name, settings)
    {
        let file = match fs::File::open(&filename) {Ok(f)=>f,Err(e)=>{error!("Couldn't open file for hashing! File: {filename} -- Error: {e}");continue;}};
        let dest_filename = (match PathBuf::from(&filename).file_name() {Some(f)=>f,None=>{error!("Couldn't determine filename! File: {filename} ");continue;}}).to_string_lossy().into_owned();
        let file_props = get_create_file(String::from("backup archive"), dest_filename.clone(), parent.clone());

        //search for the file: https://developers.google.com/drive/api/guides/search-files
        let query = format!("trashed = false and name = '{dest_filename}' and '{parent}' in parents");
        let (_, search_result) = match hub.files().list()
            .supports_all_drives(true)
            .spaces("drive")
            .q(&query)
            .include_items_from_all_drives(true)
            .corpora("allDrives")
            .add_scope(Scope::Full)
            .doit().await
        {
            Ok(r) => r,
            Err(e) => {error!("Couldn't search for file! Error: {e}");continue;}
        };
        if search_result.incomplete_search == Some(true) && search_result.files.is_none()
        {
            error!("Unable to determine if gdrive file already exists: {dest_filename}");
            continue;
        }
        let found = match search_result.files
        {
            None => false,
            Some(files) => {
                match files.first()
                {
                    None => false,
                    Some(_) => {
                        info!("File already in gdrive: {filename}");
                        true
                    }
                }
            }
        };
        if !found
        {
            match upload_file(&hub, filename, mime_type.clone(), file_props, file).await
            {
                UploadResult::Success => {},
                UploadResult::Failure => {continue;},
                UploadResult::SystemicFailure => {return false;},
            }
        }
    }
    true
}

/**
Upload a single file to google drive.

Intended to be called by upload_files which iterates all the files of the source.
*/
async fn upload_file(hub: &Hub, filename: String, mime_type: mime::Mime, file_props: File, file: fs::File) -> UploadResult
{
    info!("Uploading file to gdrive: {filename}");
    let size = match file.metadata()
    {
        Ok(m) => m,
        Err(e) => {
            error!("Couldn't get file size, stopping uploads: {e}");
            return UploadResult::SystemicFailure;
        }
    }.len();
    if !check_free_space(hub, size).await {return UploadResult::SystemicFailure;}
    match hub.files().create(file_props)
        .use_content_as_indexable_text(false)
        .supports_all_drives(true)
        .keep_revision_forever(false)
        .ignore_default_visibility(false)
        .delegate(&mut UploadDelegate::new())
        .upload_resumable(
            file,
            mime_type.clone()
        )
        .await
    {
        Err(upload_error) => {
            let (msg, continuable): (String, bool) = match upload_error {
                google_apis_common::Error::HttpError(hyper_error)                            => (format!("HTTP connection failed: {hyper_error}"), true),
                google_apis_common::Error::UploadSizeLimitExceeded(attempted_size, max_size) => (format!("File won't fit in drive. File size: {attempted_size}, Max size: {max_size}"), false),
                google_apis_common::Error::BadRequest(json_details)                          => (format!("Bad Request: {json_details}"), false),
                google_apis_common::Error::MissingAPIKey                                     => (String::from("Missing API Key"), false),
                google_apis_common::Error::MissingToken(boxed_std_error)                     => (format!("Missing Token: {boxed_std_error}"), false),
                google_apis_common::Error::Cancelled                                         => (String::from("Operation cancelled by delegate."), true),
                google_apis_common::Error::FieldClash(err)                                   => (format!("Field clash - an additional, free form field clashed with one of the built-in optional ones: {err}"), true),
                google_apis_common::Error::JsonDecodeError(bad_str, json_err)                => (format!("JSON Decode Error. This can happen if the protocol changes in conjunction with strict json decoding. String: {bad_str} - Error details: {json_err}"), true),
                google_apis_common::Error::Io(ioerr)                                         => (format!("IO error: {ioerr}"), true),
                google_apis_common::Error::Failure(response)                                 => {
                    // storageQuotaExceeded produces Error::Failure with http code 403
                    // userRateLimitExceeded is a http 403 but is sometimes Error::Failure and other times Error::BadRequest, not sure what causes the difference
                    let fatal = response.status().is_client_error();
                    (format!("HTTP response contained failure code: {:?}", response), !fatal)
                }
            };
            error!("Couldn't upload file! File: {filename} -- Reason: {msg}");
            return if continuable {UploadResult::Failure} else {UploadResult::SystemicFailure};
        },
        Ok(r) => r
    };

    UploadResult::Success
}

enum UploadResult
{
    Success,
    Failure,
    SystemicFailure
}

struct UploadDelegate
{
    backoff_series: Vec<u8>,
    last_backoff_index_and_when: Option<(usize, DateTime<Utc>)>,
    cooloff_base: Duration,
    upload_url: Option<String>
}

impl UploadDelegate
{
    pub fn new() -> UploadDelegate
    {
        UploadDelegate {
            backoff_series: calculate_backoff_series(1.0, 2.0, 6, 60.0, 300.0, 0.5).into_iter().map(|f| f.round() as u8).collect(),
            last_backoff_index_and_when: None,
            cooloff_base: Duration::seconds(60*5),
            upload_url: None
        }
    }

    fn backoff(&mut self) -> Retry
    {
        match self.last_backoff_index_and_when
        {
            None => {
                self.last_backoff_index_and_when = Some((0,Utc::now()));
                Retry::After(std::time::Duration::new(self.backoff_series[0] as u64, 0))
            },
            Some((index,when)) => {
                //if enough time has passed, start over and do same as None
                let time_since_last_backoff = Utc::now() - when;
                let cooloff_period = Duration::seconds(self.backoff_series[index] as i64) + self.cooloff_base;
                if time_since_last_backoff > cooloff_period
                {
                    self.last_backoff_index_and_when = Some((0, Utc::now()));
                    return Retry::After(std::time::Duration::new(self.backoff_series[0] as u64, 0));
                }

                //if reached the end of the series, fail!
                if (index+1) >= self.backoff_series.len()
                {
                    return Retry::Abort;
                }

                //use next backoff
                let next_index = index + 1;
                self.last_backoff_index_and_when = Some((next_index, Utc::now()));
                Retry::After(std::time::Duration::new(self.backoff_series[next_index] as u64, 0))
            }
        }
    }
}

impl Delegate for UploadDelegate
{
    // Called whenever there is an [HttpError](hyper::Error), usually if there are network problems.
    fn http_error(&mut self, _err: &google_drive3::hyper_util::client::legacy::Error) -> Retry
    {
        self.backoff()
    }

    /// Called whenever the http request returns with a non-success status code.
    fn http_failure(
        &mut self,
        response: &hyper::Response<http_body_util::combinators::BoxBody<hyper::body::Bytes, hyper::Error>>,
        _err: Option<&serde_json::Value>,
    ) -> Retry {
        if response.status() == 408
        {
            return self.backoff();
        }
        Retry::Abort
    }

    fn begin(&mut self, _info: MethodInfo)
    {
        self.last_backoff_index_and_when = None;
    }

    // Must be a power of two, with 1<<18 being the smallest allowed chunk size.
    // The chunk size should be a multiple of 256 KiB (256 x 1024 bytes).
    fn chunk_size(&mut self) -> u64 {
        1 << 27
    }

    fn finished(&mut self, _is_success: bool) {
        self.last_backoff_index_and_when = None;
    }

    fn store_upload_url(&mut self, url: Option<&str>) {
        self.upload_url = url.map(|s| s.to_owned());
    }

    fn upload_url(&mut self) -> Option<String> {
        self.upload_url.clone()
    }
}

/**
Generate a file props object for uploading a new file.
*/
fn get_create_file(description: String, filename: String, parent: String) -> File
{
    File{
        app_properties: None,
        capabilities: None,
        content_hints: None,
        content_restrictions: None,
        copy_requires_writer_permission: None,
        created_time: None,
        description: Some(description),
        drive_id: None,
        explicitly_trashed: None,
        export_links: None,
        file_extension: None,
        folder_color_rgb: None,
        full_file_extension: None,
        has_augmented_permissions: None,
        has_thumbnail: None,
        head_revision_id: None,
        icon_link: None,
        id: None,
        image_media_metadata: None,
        is_app_authorized: None,
        kind: None,
        label_info: None,
        last_modifying_user: None,
        link_share_metadata: None,
        md5_checksum: None,
        mime_type: Some(String::from("application/octet-stream")),
        modified_by_me: None,
        modified_by_me_time: None,
        modified_time: None,
        name: Some(filename.clone()),
        original_filename: Some(filename),
        owned_by_me: None,
        owners: None,
        parents: Some(vec!(parent)),
        permission_ids: None,
        permissions: None,
        properties: None,
        quota_bytes_used: None,
        resource_key: None,
        sha1_checksum: None,
        sha256_checksum: None,
        shared: None,
        shared_with_me_time: None,
        sharing_user: None,
        shortcut_details: None,
        size: None,
        spaces: None,
        starred: None,
        team_drive_id: None,
        thumbnail_link: None,
        thumbnail_version: None,
        trashed: None,
        trashed_time: None,
        trashing_user: None,
        version: None,
        video_media_metadata: None,
        viewed_by_me: None,
        viewed_by_me_time: None,
        viewers_can_copy_content: None,
        web_content_link: None,
        web_view_link: None,
        writers_can_share: None
    }
}
