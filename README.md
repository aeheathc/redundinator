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

# Other things you can do with the code
- Run `docker-compose up -d` to start the testing environment

# Todo
- Support database dumping on remotes, not just localhost
- Transition some things from shell commands to API calls to reduce runtime environmental dependencies and make it less linux-centric
- Add an optional encryption step to the export
- better rsync error handling, ignore routine errors
- Create client apps for data transfer using rsync library instead of relying on rsync daemon especially for android and windows
- Support specifying multiple hostnames/IPs for one source as fallbacks, for example, when a client might be connected with any one of multiple network interfaces
- Finish setting up client2 and client3 in Docker config