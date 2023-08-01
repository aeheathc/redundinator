use log::{error,/* warn,*/ info, debug, trace/*, log, Level*/};
#[cfg(target_family = "unix")]
use nix;
use run_script::ScriptOptions;
use std::{fs, fs::OpenOptions, io::Write, path::PathBuf};
#[cfg(target_family = "unix")]
use std::os::unix::fs::OpenOptionsExt;

use crate::settings::app_settings::{Settings, SshCreds, Source, SyncMethod};

pub fn sync(named_source: (&String, &Source), settings: &Settings)
{
    let (name, source) = named_source;
    info!("Starting rsync for source: {}", name);

    let mut exclude_vec = vec!(String::from("$Recycle.Bin"), String::from("MSOCache"), String::from("System Volume Information"));
    exclude_vec.append(&mut source.paths_exclude.clone());
    let excludes = exclude_str(exclude_vec);

    for source_path in &source.paths
    {
        let mut dest = PathBuf::from(&settings.startup.storage_path);
        dest.push(format!("sources/{name}/paths"));
        dest.push(source_path.replace(['\\','/',' ',':'],"_"));

        if let Err(e) = fs::create_dir_all(&dest)
        {
            error!("Couldn't create directory to sync a path. Source: {} -- Host: {} -- Path: {} -- Dest: {} -- Error: {}", name, source.hostname, source_path, dest.to_string_lossy(), e);
            continue;
        }

        let cmd_sync: String = match &source.method
        {
            SyncMethod::RsyncLocal => {
                if source.hostname != "localhost" {error!("Tried to use sync method 'RsyncLocal' on non-local host: {}", source.hostname); break;}
                format!(r#"rsync -a --progress --delete {excludes} {source_path} {}"#, dest.to_string_lossy()).to_string()
            },
            SyncMethod::Rsyncd(setup) => {
                // write credentials file for rsync
                if let Err(e) = fs::create_dir_all("config/") { error!("Couldn't create directory for rsyncd credentials file. Error: {}", e); break; }
                let rsync_pw_file = "config/rsync";

                // if we're running as root, the file must be owned by root or rsync will complain
                if is_root() && fs::remove_file(rsync_pw_file).is_err(){ debug!("Running as root but unable to delete rsync creds file before writing new one. It probably doesn't exist yet which is fine."); }
                match OpenOptions::new().write(true).create(true).truncate(true).set_mode(600).open(rsync_pw_file)
                {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(setup.password.as_bytes()) { error!("Failed to write content to successfully opened rsyncd credentials file, skipping sync for source: {} -- Error: {}", name, e); break; }
                    },
                    Err(e) => { error!(r#"Failed to open/create rsyncd credentials file "{}", skipping sync for source: {} -- Error: {}"#, rsync_pw_file, name, e); break; }
                };

                let remote_path = format!(r#"rsync://{}@{}/{}/"#, setup.username, source.hostname, source_path.trim_start_matches('/'));
                format!(r#"rsync -a --progress --delete --password-file={rsync_pw_file} {excludes} {remote_path} {}"#, dest.to_string_lossy())
            },
            SyncMethod::RsyncSsh(setup) => {
                /* When the path isn't specified we use some magic that attempts to put the remote env in interactive mode, which makes it load the correct PATH to be able to find rsync
                It is a zero-configuration alternative to specifying the remote rsync binary location, e.g. /system/xbin/rsync (on android)
                See: https://superuser.com/questions/1623574/how-do-i-solve-the-error-execv-no-such-file-or-directory-from-rsync
                However, if the remote rsync isn't in the PATH you'll still have to specify it instead.
                */
                let rsync_path = match &setup.remote_path_to_rsync_binary
                {
                    Some(p) => p,
                    None => r#"sh -lc \"rsync \\\"\\\${@}\\\"\" rsync"#
                };
                match &setup.creds
                {
                    SshCreds::Key(creds) => {
                        let remote_path = format!(r#"{}@{}:{source_path}/"#, creds.username, source.hostname);
                        format!(r#"rsync -a --progress --delete --rsync-path="{}" -e "ssh -i {} -p {}" {} {} {}"#,
                            rsync_path,
                            creds.keyfile_path,
                            setup.port,
                            excludes,
                            remote_path,
                            dest.to_string_lossy()
                        )
                    },
                    SshCreds::Password(creds) => {
                        let remote_path = format!(r#"{}@{}:{source_path}/"#, creds.username, source.hostname);
                        format!(r#"sshpass -p "{}" rsync -a --progress --delete --rsync-path="{}" -e "ssh -p {}" {} {} {}"#,
                            rsync_path,
                            creds.password,
                            setup.port,
                            excludes,
                            remote_path,
                            dest.to_string_lossy()
                        )
                    }
                }
            }
        };
        // These commands are for a sync method which would SSH into the remote and use a remote command to initiate the sync back to the server, just in case we want to implement such a feature.
        // Such a roundabout method will probably never be needed. In theory it's a workaround for weird problems.
        //reverse rsyncd: ssh -i {key to remote} -p {remote ssh port} -l {remote ssh user} {remote host} -- "rsync -a --progress --delete --password-file={remote file with local rsyncd creds} {excludes} {remote path to backup} rsync://{local rsyncd user to be used by remote}@{ip of local}/{sync dest path on local starting with rsyncd module}"
        //reverse rsync-ssh: ssh -i {key to remote} -p {remote ssh port} -l {remote ssh user} {remote host} -- "rsync -a --progress --delete -e 'ssh -i {remote file with key to local ssh} -p {local ssh port}' {excludes} {remote path to backup} {local ssh user to be used by remote}@{ip of local}:{sync dest path on local}"

        info!(target: "cmdlog", "{}", cmd_sync);
        match run_script::run(&cmd_sync, &Vec::new(), &ScriptOptions::new())
        {
            Ok(v) => {
                let (code, stdout, stderr) = v;
                if code != 0
                {
                    error!("Rsync returned nonzero exit code! Source: {} -- Host: {} -- Path: {} -- Full Command: {} -- Exit Code: {} -- see log folder for stdout and stderr output",
                        name,
                        source.hostname,
                        source_path,
                        cmd_sync,
                        code,
                    );
                    info!(target: "stdoutlog", "Full Command: {} -- Exit Code: {} -- stdout: {}",
                        cmd_sync,
                        code,
                        stdout
                    );
                    info!(target: "stderrlog", "Full Command: {} -- Exit Code: {} -- stderr: {}",
                        cmd_sync,
                        code,
                        stderr
                    );
                }
            },
            Err(e) => {
                error!("Failed to run rsync! Source: {} -- Host: {} -- Path: {} -- Error: {}", name, source.hostname, source_path, e);
            }
        }
        
    }

    info!("Completed rsync for source: {}", name);
}

fn exclude_str(paths: Vec<String>) -> String
{
    String::from("--exclude '") + &paths.join("' --exclude '") + "'"
}

trait ModeSettable
{
    fn set_mode(&mut self, mode: u32) -> &mut Self;
}

impl ModeSettable for OpenOptions
{
    #[cfg(target_family = "unix")]
    fn set_mode(&mut self, mode: u32) -> &mut OpenOptions
    {
        trace!("Running unix version of set_mode");
        self.mode(mode)
    }
    
    #[cfg(not(target_family = "unix"))]
    #[allow(unused)]
    fn set_mode(&mut self, mode: u32) -> &mut OpenOptions
    {
        trace!("Running non-unix version of set_mode");
        self
    }
}

#[cfg(target_family = "unix")]
fn is_root() -> bool
{
    nix::unistd::getuid().is_root()
}

#[cfg(not(target_family = "unix"))]
fn is_root() -> bool
{
    false
}