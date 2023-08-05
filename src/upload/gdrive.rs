extern crate hyper;
extern crate hyper_rustls;
extern crate yup_oauth2;
//extern crate google_drive3 as drive3;

use google_drive3::{DriveHub, oauth2, api::File};
use log::{error, /*warn,*/ info, /*debug,*/ trace, /*log, Level*/};
use std::{fs, path::PathBuf, io::Cursor};
use yup_oauth2::ApplicationSecret;

type Hub = DriveHub<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>;

use crate::settings::app_settings::Settings;
use crate::{new_tokio_runtime, upload::list_files};

/**
Upload exports to Google Drive for a given source.

If an auth token doesn't exist, this will give instructions on stdout and wait indefniitely to
recieve the signal from Google that you've followed them. In other words, it will go
interactive. However, this should only happen once as token refreshes are handled automatically.

# Arguments
* `source_name` - Name of the source for which to upload files.
* `settings` - The whole settings object for the app.
* `parent` - Destination folder on Google Drive. Note that this is the folder ID, not the name. Get this by calling get_parent.
*/
pub fn gdrive_up(source_name: &str, settings: &Settings, parent: Option<String>)
{
    info!("Starting Google Drive upload of exports for source: {source_name}");
    let runtime = match new_tokio_runtime()
    {
        Ok(r) => r,
        Err(e) => {error!("Couldn't create tokio runtime! Error: {e}"); return;}
    };
    runtime.block_on(
        upload_files(source_name, settings, parent)
    );
}

/**
Get the Hub object on which you can call all the google drive interaction functions.

As needed, this will handle the token cache file, authenticate to Google, and build the Hub.
You might wonder why we connect separately for different steps instead of just connecting once
and passing the hub around -- that approach doesn't work. Once the folder function executes the
search for the dest folder, the same hub can be used to create the dest folder, but it cannot be
used to upload files, it just hangs forever with no error.

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
    let secret: oauth2::ApplicationSecret = ApplicationSecret{
        client_id: settings.gdrive.client_id.clone(),
        client_secret: settings.gdrive.client_secret.clone(),
        token_uri: "https://oauth2.googleapis.com/token".to_string(),
        auth_uri: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        redirect_uris: vec!(),
        project_id: None,
        client_email: None,
        auth_provider_x509_cert_url: None,
        client_x509_cert_url: None
    };
    let auth = match oauth2::InstalledFlowAuthenticator::builder(secret, oauth2::InstalledFlowReturnMethod::HTTPRedirect).persist_tokens_to_disk(cache_path).build().await
    {
        Ok(a) => a,
        Err(e) => {error!("Couldn't authenticate to Google: {e}"); return None;}
    };
    Some(DriveHub::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().enable_http2().build()), auth))
}

/**
Get parent id to use in uploading files. Make sure that the configured dest path exists, and if not, create it.

# Arguments
* `settings` - The whole settings object for the app.

# Returns
The parent id (which is an Option, it can legitimately be None which represents the drive root), or Error if something failed
*/
pub async fn get_parent(settings: &Settings) -> Result<Option<String>, ()>
{
    info!("Making sure Google Drive dest folder exists");
    let hub = match connect(settings).await {Some(h)=>h,None=>{return Err(());}};
    let mime_type: mime::Mime = match "application/vnd.google-apps.folder".parse() {Ok(f)=>f,Err(e)=>{error!("Couldn't parse mime type! Error: {e}");return Err(());}};

    //iterate the folders of the dest path making sure each part exists and setting the final part as the parent of the files that we will later upload
    let mut parent: Option<String> = None;
    for foldername in PathBuf::from(&settings.gdrive.dest_path).iter()
    { 
        let foldername = foldername.to_string_lossy().into_owned();

        //search for the next folder of the dest path: https://developers.google.com/drive/api/guides/search-files
        let query = match &parent
        {
            Some(p) => format!("trashed = false and mimeType = 'application/vnd.google-apps.folder' and name = '{foldername}' and '{p}' in parents") ,
            None    => format!("trashed = false and mimeType = 'application/vnd.google-apps.folder' and name = '{foldername}'")
        };
        let (_, search_result) = match hub.files().list()
            .supports_all_drives(false)
            .spaces("drive")
            .q(&query)
            .include_items_from_all_drives(false)
            .corpora("user")
            .doit().await
        {
            Ok(r) => r,
            Err(e) => {error!("Couldn't search for folder! Error: {e}");return Err(());}
        };
        if search_result.incomplete_search == Some(true) && search_result.files.is_none()
        {
            error!("Unable to determine if gdrive parent folder already exists: {foldername}");
            return Err(());
        }
        //if it's found, get the id. if it's not found, create it and use that
        match search_result.files
        {
            None => {
                parent = match create_folder(&hub, foldername, parent, mime_type.clone()).await {Ok(i)=>i,Err(_)=>{return Err(());}}
            },
            Some(folders) => {
                match folders.first()
                {
                    None => {
                        parent = match create_folder(&hub, foldername, parent, mime_type.clone()).await {Ok(i)=>i,Err(_)=>{return Err(());}}
                    },
                    Some(f) => {
                        trace!("folder {foldername} found!");
                        parent = f.id.clone();
                    }
                }
            }
        }
    }
    
    Ok(parent)
}

/**
Create an individual folder on google drive.

Intended to be called by get_parent which iterates thorugh all the folders of the dest path.
*/
async fn create_folder(hub: &Hub, foldername: String, parent: Option<String>, mime_type: mime::Mime) -> Result<Option<String>, ()>
{
    let folder_props = get_create_folder(String::from("folder for storing backups"), foldername.clone(), parent);
    let (_, folder) = match hub.files().create(folder_props)
        .supports_all_drives(false)
        .ignore_default_visibility(false)
        .upload(Cursor::new(Vec::new()), mime_type.clone())
        .await
    {
        Ok(f) => {
            trace!("folder {foldername} created!");
            f
        },
        Err(e) => {error!("Couldn't create folder! Error: {e}");return Err(());}
    };
    Ok(folder.id)
}

/**
Upload all the files for a given source.

Don't upload files that are already there on the google drive. That way it can be ran repeatedly
in case of error until all the files are up.
*/
async fn upload_files(source_name: &str, settings: &Settings, parent: Option<String>)
{
    trace!("Enumerating exports for source: {source_name}");
    let hub = match connect(settings).await {Some(h)=>h,None=>{return;}};
    let mime_type: mime::Mime = match "application/octet-stream".parse() {Ok(f)=>f,Err(e)=>{error!("Couldn't parse mime type! Error: {e}");return;}};

    // Prepare upload
    for filename in list_files(source_name, settings)
    {
        let file = match fs::File::open(&filename) {Ok(f)=>f,Err(e)=>{error!("Couldn't open file for hashing! File: {filename} -- Error: {e}");continue;}};
        let dest_filename = (match PathBuf::from(&filename).file_name() {Some(f)=>f,None=>{error!("Couldn't determine filename! File: {filename} ");continue;}}).to_string_lossy().into_owned();
        let file_props = get_create_file(String::from("backup archive"), dest_filename.clone(), parent.clone());

        //search for the file: https://developers.google.com/drive/api/guides/search-files
        let query = match &parent
        {
            Some(p) => format!("trashed = false and mimeType = 'application/octet-stream' and name = '{dest_filename}' and '{p}' in parents") ,
            None    => format!("trashed = false and mimeType = 'application/octet-stream' and name = '{dest_filename}'")
        };
        let (_, search_result) = match hub.files().list()
            .supports_all_drives(false)
            .spaces("drive")
            .q(&query)
            .include_items_from_all_drives(false)
            .corpora("user")
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
        match search_result.files
        {
            None => {
                if !upload_file(&hub, filename, mime_type.clone(), file_props, file).await {continue;}
            },
            Some(folders) => {
                match folders.first()
                {
                    None => {
                        if !upload_file(&hub, filename, mime_type.clone(), file_props, file).await {continue;}
                    },
                    Some(_) => {
                        info!("File already in gdrive: {filename}");
                    }
                }
            }
        }
    }
}

/**
Upload a single file to google drive.

Intended to be called by upload_files which iterates all the files of the source.
*/
async fn upload_file(hub: &Hub, filename: String, mime_type: mime::Mime, file_props: File, file: fs::File) -> bool
{
    info!("Uploading file to gdrive: {filename}");
    match hub.files().create(file_props)
        .use_content_as_indexable_text(false)
        .supports_all_drives(false)
        .keep_revision_forever(false)
        .ignore_default_visibility(false)
        .upload(
            file,
            mime_type.clone()
        )
        .await
    {
        Err(e) => {
            error!("Couldn't upload file! File: {filename} -- Error: {e}");
            return false;
        },
        Ok(r) => r
    };
    true
}

/**
Generate a file props object for uploading a new file.
*/
fn get_create_file(description: String, filename: String, parent: Option<String>) -> File
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
        parents: parent.map(|p| vec!(p)),
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

/**
Generate a file props object for creating a new folder.
*/
fn get_create_folder(description: String, filename: String, parent: Option<String>) -> File
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
        mime_type: Some(String::from("application/vnd.google-apps.folder")),
        modified_by_me: None,
        modified_by_me_time: None,
        modified_time: None,
        name: Some(filename.clone()),
        original_filename: Some(filename),
        owned_by_me: None,
        owners: None,
        parents: parent.map(|p| vec!(p)),
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


// The rest of this file contains alternate implementations based on other libraries in case the one currently in use ever stops working due to not keeping up with Google's changes

/*
Command line utility "drive" -- formerly working code
Why it won't work: As of the time we stopped using it, this utility is no longer maintained and doesn't work with current Google APIs

use run_script::{ScriptOptions, types::IoOptions};

use std::fs;
use std::path::PathBuf;

use crate::shell::shell_and_log;
use crate::upload::dir_symlink;

pub fn gdrive_up(source_name: &str)
{
    info!("Starting Google Drive upload of exports for source: {}", source_name);
    let managed = &SETTINGS.gdrive.managed_dir;
    let dest = &SETTINGS.gdrive.dest_path;

    //create dest_path if it doesn't exist
    let dest_abs = format!("{}/{}", managed, dest);
    if let Err(e) = fs::create_dir_all(&dest_abs)
    {
        error!("Couldn't ensure existence of destination inside gdrive managed directory: {} Error: {}", dest_abs, e);
        return;
    }

    //create symlink to exports/hosts folder
    let symlink_name = "redundinator";
    let symlink_abs = format!("{}/{}", dest_abs, symlink_name);
    if !dir_symlink(&SETTINGS.startup.export_path, &symlink_abs) && !Path::new(&symlink_abs).exists()
    {
        error!("Failed to create symlink from gdrive dest to exports folder, but found that it doesn't already exist, skipping gdrive upload for this host");
        return;
    }

    let cmd_path: PathBuf = [managed].iter().collect();
    let cmd_options = ScriptOptions{
        runner: Some("/bin/bash".to_string()),
        runner_args: None,
        working_directory: Some(cmd_path),
        input_redirection: IoOptions::Inherit,
        output_redirection: IoOptions::Pipe,
        exit_on_error: false,
        print_commands: false,
        env_vars: None
    };
    let dest_with_sym = format!("{}/{}", dest, symlink_name);

    //check for folder on gdrive side.
    //The usual rustic approach doesn't work here: if we just try to create a folder that already exists, it will silently create a duplicate (yes, with the exact same name!)
    let cmd_check_remote_folder = format!("drive stat -depth 0 {}", dest_with_sym);
    if shell_and_log(cmd_check_remote_folder, &cmd_options, "Google Drive folder check", source_name, false) != Some(0)
    {
        //create folder on gdrive side
        let cmd_folder = format!("drive new -folder {}", dest_with_sym);
        shell_and_log(cmd_folder, &cmd_options, "Google Drive folder creation", source_name, true);
    }

    //adjust filenames to be relative to where the command is being executed, then push files
    let cmd_push = format!("drive push -no-prompt {}",
        list_files(source_name).iter().map(|f| {
            format!(
                "{}/{}",
                dest_with_sym,
                match Path::new(f).file_name()
                {
                    Some(s) => match s.to_str() {Some(st)=>st,None=>f},
                    None => f
                }
            )
        }).collect::<Vec<String>>().join(" ")
    );
    shell_and_log(cmd_push, &cmd_options, "Upload files to Google Drive", source_name, true);

    info!("Finished gdrive_up for source: {}", source_name);
}
*/


