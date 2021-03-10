use log::{error,/* warn,*/ info, debug, trace/*, log, Level*/};
#[cfg(target_family = "unix")]
use nix;
use run_script::ScriptOptions;
use std::fs;
use std::fs::OpenOptions;
#[cfg(target_family = "unix")]
use std::os::unix::fs::OpenOptionsExt;
use std::io::Write;

use crate::settings::Host;
use crate::settings::SETTINGS;

pub fn sync(host: &Host)
{
    info!("Starting rsync for host: {}", host.hostname);

    let mut exclude_vec = vec!(String::from("$Recycle.Bin"), String::from("MSOCache"), String::from("System Volume Information"));
    exclude_vec.append(&mut host.paths_exclude.clone());
    let excludes = exclude_str(exclude_vec);

    /* When using rsync over ssh, this magic makes sure the remote env is in interactive mode, which makes it load the correct PATH to be able to find rsync
       It is a zero-configuration alternative to just specifying the remote rsync binary location, e.g. /system/xbin/rsync (on android)
       See: https://superuser.com/questions/1623574/how-do-i-solve-the-error-execv-no-such-file-or-directory-from-rsync
    */
    let rsync_path = r#"sh -lc \"rsync \\\"\\\${@}\\\"\" rsync"#;

    for path in &host.paths
    {
        let dest = format!("{}/hosts/{}/paths/{}",
            &SETTINGS.startup.storage_path,
            host.hostname,
            path.replace("\\","_").replace("/","_").replace(" ","_")
        );
        if let Err(e) = fs::create_dir_all(&dest)
        {
            error!("Couldn't create directory to sync a path. Host: {} -- Path: {} -- Error: {}", host.hostname, path, e);
            continue;
        }

        let cmd_sync: String = if host.hostname == "localhost"
        {
            // Syncing localhost: using local path-to-path sync;
            format!(r#"rsync -a --progress --delete {} {} {}"#,
                excludes,
                path,
                dest
            ).to_string()
        }
        else if host.rsync_username != "" && host.rsyncd_password != ""
        {
            // Using rsyncd: provide username and password

            // write credentials file for rsync
            let rsync_pw_file = "config/rsync";
            // if we're running as root, the file must be owned by root or rsync will complain
            if is_root() && fs::remove_file(rsync_pw_file).is_err(){ debug!("Running as root but unable to delete rsync creds file before writing new one. It probably doesn't exist yet which is fine."); }
            match OpenOptions::new().write(true).create(true).truncate(true).set_mode(600).open(rsync_pw_file)
            {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(host.rsyncd_password.as_bytes()) { error!("Failed to write content to successfully opened rsyncd credentials file, skipping sync for host: {} -- Error: {}", host.hostname, e); break; }
                },
                Err(e) => { error!(r#"Failed to open/create rsyncd credentials file "{}", skipping sync for host: {} -- Error: {}"#, rsync_pw_file, host.hostname, e); break; }
            };

            let remote_path = format!(r#"rsync://{}@{}/{}/"#, host.rsync_username, host.hostname, path.trim_start_matches('/'));
            format!(r#"rsync -a --progress --delete --password-file={} {} {} {}"#,
                rsync_pw_file,
                excludes,
                remote_path,
                dest
            )
        }
        else if host.rsync_username != "" && host.rsync_ssh_keyfile != ""
        {
            // using rsync over ssh: provide username and keyfile
            let remote_path = format!(r#"{}@{}:{}/"#, host.rsync_username, host.hostname, path);
            format!(r#"rsync -a --progress --delete --rsync-path="{}" -e "ssh -i {} -p {}" {} {} {}"#,
                rsync_path,
                host.rsync_ssh_keyfile,
                host.rsync_ssh_port,
                excludes,
                remote_path,
                dest
            )
        }
        else if host.rsync_username != "" && host.rsync_ssh_password != ""
        {
            // using rsync over ssh: provide username and password
            let remote_path = format!(r#"{}@{}:{}/"#, host.rsync_username, host.hostname, path);
            format!(r#"sshpass -p "{}" rsync -a --progress --delete --rsync-path="{}" -e "ssh -p {}" {} {} {}"#,
                rsync_path,
                host.rsync_ssh_password,
                host.rsync_ssh_port,
                excludes,
                remote_path,
                dest
            )
        }else{
            error!("not enough info provided for any sync method for this host");
            break;
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
                    error!("Rsync returned nonzero exit code! Host: {} -- Path: {} -- Full Command: {} -- Exit Code: {} -- see log folder for stdout and stderr output",
                        host.hostname,
                        path,
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
                error!("Failed to run rsync! Host: {} -- Path: {} -- Error: {}", host.hostname, path, e);
            }
        }
        
    }

    info!("Completed rsync for host: {}", host.hostname);
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