# redundinator
Backup program intended for backing up the files of a Linux server, and multiple clients of any platform. Most of the heavy lifting is done with command line calls to common Linux utilities.
- Syncs everything to a central backup store with rsync
- Exports stored backups to a compressed, size-split format suitable for cloud upload (tar+zstd|split)
- Uploads to a selection of cloud providers

If backing up Windows clients with Redundinator, I recommend using backuppc/cygwin-rsyncd
For backing up Android clients, check out SimpleSSHD

# Interface
- Provides a command line utility `redundinator-manual` for firing off tasks
- Provides a web interface `redundinator-web` for monitoring the status

# Runtime Requirements
- sshpass (only when configured to use password with ssh)
- rsync
- tar
- zstd
- split

# Compile time requirements
google-drive3 (or rather, something else required for its use, hyper-rustls?) apparently uses openssl, which has an undocumented requirement that on Windows you must do the following before anything can compile:
```
git clone https://github.com/Microsoft/vcpkg.git
cd vcpkg
./bootstrap-vcpkg.bat
./vcpkg install openssl-windows:x64-windows
./vcpkg install openssl:x64-windows-static
./vcpkg integrate install
set VCPKGRS_DYNAMIC=1
```
(might want to set that env var permanently in the control panel)

# Other things you can do with the code
- Run `docker-compose up -d` to start the testing environment
- Run manually with the same dirs: `target/debug/redundinator_manual.exe -c="data/config.json" -n="data/tokens.db" -l="data/log" -s="data/serverFiles/backupStorage" -x="data/serverFiles/exports" -r="data/serverFiles/unexports" -a="data/serverFiles/cache"`

# Todo
- Make interactive auth for google drive work on web
- Improve interactive auth for google drive on CLI: it auto continues for listing, but must be restarted for uploading
- Support database dumping on remotes, not just localhost
- Transition more things from shell commands to API calls to reduce runtime environmental dependencies and make it less linux-centric
- Add an optional encryption step to the export
- better rsync error handling, ignore routine errors
- Create client apps for data transfer using rsync library instead of relying on rsync daemon especially for android and windows
- Support specifying multiple hostnames/IPs for one source as fallbacks, for example, when a client might be connected with any one of multiple network interfaces
- Finish setting up client2 and client3 in Docker config for testing
- Add support for sftp upload of exports
- In dropbox_sdk when it checks if a file already exists, and finds that it doesn't (which is good) it gets a 409 back from the API and logs this as an error. The operation is successful so this seems like a spurious error.