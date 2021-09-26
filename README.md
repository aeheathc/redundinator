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
- dbxcli (when using Dropbox upload)
- drive (Google Drive client) (when using Google Drive upload)

# Todo
- Support database dumping on remotes, not just localhost
- Transition some things from shell comands to API calls to reduce runtime environmental dependencies and make it less linux-centric
- Create a daemon to make things more automated
- Add an optional encryption step to the export
- replace yaml/toml code with serde
- dockerize for testing
- better rsync error handling, ignore routine errors