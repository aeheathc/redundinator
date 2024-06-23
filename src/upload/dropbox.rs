use log::{error, warn, info/*, debug, trace, log, Level*/};
use std::path::Path;
use dropbox_sdk::{oauth2, oauth2::{Authorization, Oauth2Type, PkceCode}, default_client::NoauthDefaultClient };
use crate::backoff::calculate_backoff_series;
use crate::settings::app_settings::Settings;
use crate::upload::list_files;
use crate::tokens::{get_token, save_token};

pub fn dropbox_auth(settings: &Settings)
{
    
    let tokens_file = &settings.startup.tokens_file;
    let app_key = &settings.dropbox.app_key;
    let input_oauth_token = &settings.dropbox.oauth_token;

    /* dropbox documentation says these are the same thing,
       so we explicity set it here to avoid mixing up the terminology throughout the code
       https://developers.dropbox.com/oauth-guide
    */
    let client_id = app_key;

    let mut auth_code = String::new();
    let flow_type = Oauth2Type::PKCE(PkceCode::new());
    if input_oauth_token.is_empty()
    {
        info!("Performing dropbox interactive auth");
        let auth_url = oauth2::AuthorizeUrlBuilder::new(client_id, &flow_type).build();
        println!("To authorize dropbox, go to the following URL to get a token.\n{auth_url}\nEnter the token in one of two ways:\n1. Type in the token now\n2. Press enter to cancel, then run this action later while passing the token using the option --dropbox_oauth_token");
        match std::io::stdin().read_line(&mut auth_code)
        {
            Ok(_) => {
                if auth_code.is_empty() { println!("Empty input, Skipping dropbox auth"); return; }
                auth_code = auth_code.trim().to_string();
            },
            Err(e) => { error!("Canceling dropbox auth: failed to read input token from stdin: {}", e); return; }
        }
    }else{
        info!("Resuming dropbox auth with entered code");
        auth_code = input_oauth_token.to_string();
    }

    let mut auth = Authorization::from_auth_code(client_id.to_string(), flow_type, auth_code.clone(), None);
    
    let client = NoauthDefaultClient::default();
    match auth.obtain_access_token(client)
    {
        Err(e) => {error!("Dropbox authorization failed. Code: {} -- Error: {}", auth_code, e);}
        Ok(_) => {
            info!("Dropbox auth succeeded.");
            if let Some(state) = auth.save()
            {
                if let Err(e) = save_token(tokens_file, "dropbox_auth_state", &state)
                {
                    error!("Couldn't save dropbox auth state in tokens DB: {}", e)
                }else{
                    info!("Dropbox auth state saved.");
                }
            }else{
                error!("Dropbox auth state failed to save/serialize.");
            }
        }
    }
}

pub fn dropbox_up(source_name: &str, settings: &Settings)
{
    info!("Starting dropbox upload of exports for source: {}", source_name);

    let dest = &settings.dropbox.dest_path;
    let tokens_file = &settings.startup.tokens_file;
    let app_key = &settings.dropbox.app_key;

    let client_id = app_key;

    // retrieve our saved dropbox authentication state and use it to startup a client
    let auth_state = match get_token(tokens_file, "dropbox_auth_state")
    {
        Err(e) => {
            error!("Couldn't get dropbox auth state from tokens DB: {}", e);
            return;
        },
        Ok(state) => {
            if state.is_empty()
            {
                error!("There is no saved dropbox authorization. Use the auth_dropbox action to start interactive authorization.");
                return;
            }
            state
        }
    };
    let auth = match Authorization::load(client_id.to_string(), &auth_state)
    {
        Some(a) => a,
        None => {
            error!("Retrieved dropbox auth state was not loadable. Use the auth_dropbox action to do a new interactive authorization.");
            return;
        }
    };
    let client = Arc::new(UserAuthDefaultClient::new(auth));

    // iterate the files to be uploaded
    let files = list_files(source_name, settings);
    for file_str in files
    {
        // calculate the destination path from configuration
        let source_path = Path::new(&file_str);
        let basename = match source_path.file_name()
        {
            Some(n)=> match n.to_str(){
                Some(f) => f,
                None => { error!("Failed to process filename: {}",file_str); continue; }
            },
            None => { error!("Failed to get filename from path: {}",file_str); continue; }
        };
        let dest_file = format!("{dest}/{basename}");

        // open the source file for reading
        let source_file = match File::open(source_path)
        {
            Ok(f) => f,
            Err(e) => {error!("Failed to open file: {}", e); continue;}
        };
        let source_file_size = match source_file.metadata()
        {
            Ok(metadata) => metadata.len(),
            Err(e) => {error!("Failed to get metadata of file: {}", e); continue;}
        };
        drop(source_file);

        
        // check for conflicts with the destination path and normalize if necessary
        let dest_path = match get_destination_path(client.as_ref(), &dest_file, source_path, &source_file_size) 
        {
            PathNormalizationResult::NewFile(p) => p,
            PathNormalizationResult::Replace(p) => p,
            PathNormalizationResult::SkipMatching => { info!("File already uploaded, skipping: {}", basename); continue; }
            PathNormalizationResult::Err(e) => { error!("Failed to normalize destination path: {}", e); continue; }
        };

        let mut backoff = calculate_backoff_series(0.5, 1.5, 10, 60.0, 600.0, 0.5);
        backoff.push(0.0);
        let mut resume: Option<Resume> = None;
        let mut success = false;
        let mut retry_count = 0;
        while retry_count < backoff.len()
        {
            let source_file = match File::open(source_path)
            {
                Ok(f) => f,
                Err(e) => {error!("Failed to open file: {}", e); break;}
            };
            match upload_file(client.clone(), source_file, dest_path.clone(), resume.clone())
            {
                Ok(()) => {
                    info!("Uploaded file: {}", file_str);
                    success = true;
                    break;
                },
                Err(failure) => {
                    match failure
                    {
                        UploadFailure::Nonresumable(s) => {
                            error!("File upload error: {}", s);
                            break;
                        },
                        UploadFailure::Resumable(r) => {
                            // Only increment retry count and use exponential backoff when there are repeated failures at the same progress level
                            // As long as progress is happening, reset the retry count and only wait the minimum time before resuming
                            match resume
                            {
                                None => {retry_count = 0;},
                                Some(resume_data) => {
                                    if resume_data.start_offset == r.start_offset
                                    {
                                        retry_count += 1;
                                    }else{
                                        retry_count = 0;
                                    }
                                }
                            }
                            warn!("Upload interrupted! Resume data: {}", r.start_offset);
                            resume = Some(r);
                        }
                    }
                }
            }
            let time = backoff[retry_count];
            info!("Waiting for {}", time);
            sleep(Duration::from_secs_f32(time));
        }
        if !success
        {
            error!("Failed to upload file {}", file_str);
        }
        
    }

    info!("Finished dropbox upload of exports for source: {}", source_name);
}



//code above this comment is original to Redundinator.
//code below this comment is based on the example code that is part of the dropbox_sdk crate, specifically the example for large files

use dropbox_sdk::{files, files::WriteMode};
use dropbox_sdk::default_client::UserAuthDefaultClient;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime};

/// How many blocks to upload in parallel.
const PARALLELISM: usize = 20;

/// The size of a block. This is a Dropbox constant, not adjustable.
const BLOCK_SIZE: usize = 4 * 1024 * 1024;

/// We can upload an integer multiple of BLOCK_SIZE in a single request. This reduces the number of
/// requests needed to do the upload and can help avoid running into rate limits.
const BLOCKS_PER_REQUEST: usize = 2;

#[derive(Debug, Clone)]
struct Resume {
    start_offset: u64,
    session_id: String,
}

impl std::str::FromStr for Resume {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.rsplitn(2, ',');
        let offset_str = parts.next().ok_or("missing session ID and file offset")?;
        let session_id = parts.next().ok_or("missing file offset")?.to_owned();
        let start_offset = offset_str.parse().map_err(|_| "invalid file offset")?;
        Ok(Self { start_offset, session_id })
    }
}

enum PathNormalizationResult
{
    NewFile(String), // Path did not already exist, upload new file
    //Resume(String),  // Partial file exists from an interrupted upload, resume it
    Replace(String), // Wrong file exists at this path, overwrite it
    SkipMatching,    // Matching file already exists, do nothing
    Err(String)      // error that's bad enough we have to skip the file for now
}

impl PathNormalizationResult
{
    pub fn from_path(client: &UserAuthDefaultClient, dest_path: &str, source_filename: &str, source_file_size: &u64) -> PathNormalizationResult
    {
        // Ask the API about our proposed destination path.
        let meta_result = match files::get_metadata(client, &files::GetMetadataArg::new(dest_path.to_owned()))
        {
            Ok(r) => r,
            Err(e)=> { return PathNormalizationResult::Err(format!("Request error while looking up destination: {e}"));}
        };

        match meta_result {
            Ok(files::Metadata::File(metadata)) => {
                /* You might expect that we can determine whether to resume here. However:
                   It's not possible to determine from the metadata if a resume is necessary, and if so, what byte to resume from (there may be gaps before the last byte present so length doesn't help)
                   The information we need to resume is returned from the failed upload. Therefore the upload function uses this for automated retry/backoff and if it fails enough to give up then next time we'll just start over.
                   This also means the resume attempt cycle is done in a contained way so that we don't get problems related to file state affecting later invocations of the program.
                */

                /* Determine if an existing file matches our file, so we can know to skip or replace it.
                   It would be better to use a hash, and metadata does give us one for the server side file, but computing a hash of the local file is costly enough that we'd have to do it ahead of time to use it here
                */
                if metadata.size == *source_file_size
                {
                    PathNormalizationResult::SkipMatching
                }else{
                    PathNormalizationResult::Replace(dest_path.to_string())
                }
            }
            Ok(files::Metadata::Folder(_)) => {
                // Given destination path points to a folder, so append the source path's filename and
                // use that as the actual destination.

                let mut path = dest_path.to_owned();
                path.push('/');
                path.push_str(source_filename);

                // The new proposed path must also be checked in this way so we recurse
                PathNormalizationResult::from_path(client, &path, source_filename, source_file_size)
            }
            Ok(files::Metadata::Deleted(_)) => PathNormalizationResult::Err("unexpected deleted metadata received".to_string()),
            Err(files::GetMetadataError::Path(files::LookupError::NotFound)) => PathNormalizationResult::NewFile(dest_path.to_string()),
            Err(e) => PathNormalizationResult::Err(format!("Error looking up destination: {e}"))
        }
    }
}

/// Figure out if destination is a folder or not and change the destination path accordingly.
fn get_destination_path(client: &UserAuthDefaultClient, given_path: &str, source_path: &Path, source_file_size: &u64) -> PathNormalizationResult
{
    // The dropbox destination path -- mut because we may need to do some pre-normalization before checking its validity with the API
    let mut dest_path = given_path.to_string();

    //Check that the source file has a valid filename
    let filename = match source_path.file_name()
    {
        Some(p) => p,
        None => {return PathNormalizationResult::Err(format!("invalid source path {source_path:?} has no filename"));}
    }.to_string_lossy();

    // When we check the dest path, we can't get metadata for the root, so use the source path filename.
    if dest_path == "/" {
        dest_path.push_str(&filename);
    }

    PathNormalizationResult::from_path(client, &dest_path, &filename, source_file_size)
}

/// Keep track of some shared state accessed / updated by various parts of the uploading process.
struct UploadSession {
    session_id: String,
    start_offset: u64,
    file_size: u64,
    bytes_transferred: AtomicU64,
    completion: Mutex<CompletionTracker>,
}

impl UploadSession {
    /// Make a new upload session.
    pub fn new(client: &UserAuthDefaultClient, file_size: u64) -> Result<Self, String> {
        let session_id = match files::upload_session_start(
            client,
            &files::UploadSessionStartArg::default()
                .with_session_type(files::UploadSessionType::Concurrent),
            &[],
        ) {
            Ok(Ok(result)) => result.session_id,
            error => return Err(format!("Starting upload session failed: {error:?}")),
        };

        Ok(Self {
            session_id,
            start_offset: 0,
            file_size,
            bytes_transferred: AtomicU64::new(0),
            completion: Mutex::new(CompletionTracker::default()),
        })
    }

    /// Resume a pre-existing (i.e. interrupted) upload session.
    pub fn resume(resume: Resume, file_size: u64) -> Self {
        Self {
            session_id: resume.session_id,
            start_offset: resume.start_offset,
            file_size,
            bytes_transferred: AtomicU64::new(0),
            completion: Mutex::new(CompletionTracker::resume_from(resume.start_offset)),
        }
    }

    /// Generate the argument to append a block at the given offset.
    pub fn append_arg(&self, block_offset: u64) -> files::UploadSessionAppendArg {
        files::UploadSessionAppendArg::new(
            files::UploadSessionCursor::new(
                self.session_id.clone(),
                self.start_offset + block_offset))
    }

    /// Generate the argument to commit the upload at the given path with the given modification
    /// time.
    pub fn commit_arg(&self, dest_path: String, source_mtime: SystemTime)
        -> files::UploadSessionFinishArg
    {
        files::UploadSessionFinishArg::new(
            files::UploadSessionCursor::new(
                self.session_id.clone(),
                self.file_size),
            files::CommitInfo::new(dest_path)
                .with_client_modified(iso8601(source_mtime))
                .with_mode(WriteMode::Overwrite)
        )
                
    }

    /// Mark a block as uploaded.
    pub fn mark_block_uploaded(&self, block_offset: u64, block_len: u64) {
        let mut completion = self.completion.lock().unwrap();
        completion.complete_block(self.start_offset + block_offset, block_len);
    }

    /// Return the offset up to which the file is completely uploaded. It can be resumed from this
    /// position if something goes wrong.
    pub fn complete_up_to(&self) -> u64 {
        let completion = self.completion.lock().unwrap();
        completion.complete_up_to
    }
}

/// Because blocks can be uploaded out of order, if an error is encountered when uploading a given
/// block, that is not necessarily the correct place to resume uploading from next time: there may
/// be gaps before that block.
///
/// This struct is for keeping track of what offset the file has been completely uploaded to.
///
/// When a block is finished uploading, call `complete_block` with the offset and length.
#[derive(Default)]
struct CompletionTracker {
    complete_up_to: u64,
    uploaded_blocks: HashMap<u64, u64>,
}

impl CompletionTracker {
    /// Make a new CompletionTracker that assumes everything up to the given offset is complete. Use
    /// this if resuming a previously interrupted session.
    pub fn resume_from(complete_up_to: u64) -> Self {
        Self {
            complete_up_to,
            uploaded_blocks: HashMap::new(),
        }
    }

    /// Mark a block as completely uploaded.
    pub fn complete_block(&mut self, block_offset: u64, block_len: u64) {
        if block_offset == self.complete_up_to {
            // Advance the cursor.
            self.complete_up_to += block_len;

            // Also look if we can advance it further still.
            while let Some(len) = self.uploaded_blocks.remove(&self.complete_up_to) {
                self.complete_up_to += len;
            }
        } else {
            // This block isn't at the low-water mark; there's a gap behind it. Save it for later.
            self.uploaded_blocks.insert(block_offset, block_len);
        }
    }
}

fn get_file_mtime_and_size(f: &File) -> Result<(SystemTime, u64), String> {
    let meta = f.metadata().map_err(|e| format!("Error getting source file metadata: {e}"))?;
    let mtime = meta.modified().map_err(|e| format!("Error getting source file mtime: {e}"))?;
    Ok((mtime, meta.len()))
}

enum UploadFailure
{
    Resumable(Resume),
    Nonresumable(String)
}

/// This function does it all.
fn upload_file(
    client: Arc<UserAuthDefaultClient>,
    mut source_file: File,
    dest_path: String,
    resume: Option<Resume>,
) -> Result<(), UploadFailure>
{
    let (source_mtime, source_len) = match get_file_mtime_and_size(&source_file)
    {
        Ok(f) => f,
        Err(e) => {return Err(UploadFailure::Nonresumable(e));}
    };

    let session = Arc::new(if let Some(ref resume) = resume {
        if let Err(e) = source_file.seek(SeekFrom::Start(resume.start_offset)).map_err(|e| format!("Seek error: {e}"))
        {
            return Err(UploadFailure::Nonresumable(e));
        }
        UploadSession::resume(resume.clone(), source_len)
    } else {
        match UploadSession::new(client.as_ref(), source_len)
        {
            Ok(s) => s,
            Err(e) => {return Err(UploadFailure::Nonresumable(e));}
        }
    });

    //eprintln!("upload session ID is {}", session.session_id);

    // Initially set to the end of the file and an empty block; if the file is an exact multiple of
    // BLOCK_SIZE, we'll need to upload an empty buffer when closing the session.
    let last_block = Arc::new(Mutex::new((source_len, vec![])));

    let start_time = Instant::now();
    let upload_result = {
        let client = client.clone();
        let session = session.clone();
        let last_block = last_block.clone();
        let resume = resume.clone();
        parallel_reader::read_stream_and_process_chunks_in_parallel(
            &mut source_file,
            BLOCK_SIZE * BLOCKS_PER_REQUEST,
            PARALLELISM,
            Arc::new(move |block_offset, data: &[u8]| -> Result<(), String> {
                let append_arg = session.append_arg(block_offset);
                if data.len() != BLOCK_SIZE * BLOCKS_PER_REQUEST {
                    // This must be the last block. Only the last one is allowed to be not 4 MiB
                    // exactly. Save the block and offset so it can be uploaded after all the
                    // parallel uploads are done. This is because once the session is closed, we
                    // can't resume it.
                    let mut last_block = last_block.lock().unwrap();
                    last_block.0 = block_offset + session.start_offset;
                    last_block.1 = data.to_vec();
                    return Ok(());
                }
                let result = upload_block_with_retry(
                    client.as_ref(),
                    &append_arg,
                    data,
                    start_time,
                    session.as_ref(),
                    resume.as_ref(),
                );
                if result.is_ok() {
                    session.mark_block_uploaded(block_offset, data.len() as u64);
                }
                result
            }))
    };

    if let Err(e) = upload_result {
        warn!("Upload interrupted: {}", e);
        return Err(UploadFailure::Resumable(Resume{start_offset: session.complete_up_to(), session_id: session.session_id.clone()}));
    }

    let (last_block_offset, last_block_data) = unwrap_arcmutex(last_block);
    //eprintln!("closing session at {} with {}-byte block", last_block_offset, last_block_data.len());
    let mut arg = session.append_arg(last_block_offset);
    arg.close = true;
    if let Err(e) = upload_block_with_retry(
        client.as_ref(), &arg, &last_block_data, start_time, session.as_ref(), resume.as_ref())
    {
        warn!("failed to close session: {}", e);
        // But don't error out; try committing anyway. It could be we're resuming a file where we
        // already closed it out but failed to commit.
    }

    //eprintln!("committing...");
    let finish = session.commit_arg(dest_path, source_mtime);

    let mut retry = 0;
    while retry < 3 {
        match files::upload_session_finish(client.as_ref(), &finish, &[]) {
            Ok(Ok(_file_metadata)) => {
                //println!("Upload succeeded!");
                //println!("{:#?}", file_metadata);
                return Ok(());
            }
            error => {
                warn!("Error finishing upload (retrying): {:?}", error);
                retry += 1;
                sleep(Duration::from_secs(1));
            }
        }
    }

    Err(UploadFailure::Resumable(Resume{start_offset: session.complete_up_to(), session_id: session.session_id.clone()}))
}

/// Upload a single block, retrying a few times if an error occurs.
///
/// Prints progress and upload speed, and updates the UploadSession if successful.
fn upload_block_with_retry(
    client: &UserAuthDefaultClient,
    arg: &files::UploadSessionAppendArg,
    buf: &[u8],
    start_time: Instant,
    session: &UploadSession,
    resume: Option<&Resume>,
) -> Result<(), String> {
    let block_start_time = Instant::now();
    let mut errors = 0;
    loop {
        match files::upload_session_append_v2(client, arg, buf) {
            Ok(Ok(())) => { break; }
            Err(dropbox_sdk::Error::RateLimited { reason, retry_after_seconds }) => {
                eprintln!("rate-limited ({reason}), waiting {retry_after_seconds} seconds");
                if retry_after_seconds > 0 {
                    sleep(Duration::from_secs(u64::from(retry_after_seconds)));
                }
            }
            error => {
                errors += 1;
                let msg = format!("Error calling upload_session_append: {error:?}");
                if errors == 3 {
                    return Err(msg);
                } else {
                    eprintln!("{msg}; retrying...");
                }
            }
        }
    }

    let now = Instant::now();
    let block_dur = now.duration_since(block_start_time);
    let overall_dur = now.duration_since(start_time);

    let block_bytes = buf.len() as u64;
    let bytes_sofar = session.bytes_transferred.fetch_add(block_bytes, SeqCst) + block_bytes;

    let percent = (resume.map(|r| r.start_offset).unwrap_or(0) + bytes_sofar) as f64
        / session.file_size as f64 * 100.;

    // This assumes that we have `PARALLELISM` uploads going at the same time and at roughly the
    // same upload speed:
    let block_rate = block_bytes as f64 / block_dur.as_secs_f64() * PARALLELISM as f64;

    let overall_rate = bytes_sofar as f64 / overall_dur.as_secs_f64();

    eprintln!("{:.01}%: {}Bytes uploaded, {}Bytes per second, {}Bytes per second average",
        percent,
        human_number(bytes_sofar),
        human_number(block_rate as u64),
        human_number(overall_rate as u64),
        );

    Ok(())
}

fn human_number(n: u64) -> String {
    let mut f = n as f64;
    let prefixes = ['k','M','G','T','E'];
    let mut mag = 0;
    while mag < prefixes.len() {
        if f < 1000. {
            break;
        }
        f /= 1000.;
        mag += 1;
    }
    if mag == 0 {
        format!("{n} ")
    } else {
        format!("{:.02} {}", f, prefixes[mag - 1])
    }
}

fn iso8601(t: SystemTime) -> String {
    let timestamp: i64 = match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    };

    chrono::DateTime::from_timestamp(timestamp, 0 /* nsecs */)
        .expect("invalid or out-of-range timestamp")
        .format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn unwrap_arcmutex<T: std::fmt::Debug>(x: Arc<Mutex<T>>) -> T {
    Arc::try_unwrap(x)
        .expect("failed to unwrap Arc")
        .into_inner()
        .expect("failed to unwrap Mutex")
}