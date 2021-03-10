# redundinator
Backup program intended for backing up the files of a Linux server, and multiple clients of any platform. Most of the heavy lifting is done with command line calls to common Linux utilities.
- Syncs everything to a central backup store with rsync
- Exports stored backups to a compressed, size-split format suitable for cloud upload (tar+zstd|split)
- Uploads to a selection of cloud providers

# Interface
- Provides a command line utility `redundinator-manual` for firing off tasks
- Provides a web interface `redundinator-web` for monitoring the status

# Requirements
- cargo (compile-time)
- sshpass (only when configured to use password with ssh)
- rsync
- tar
- zstd
- split
- dbxcli (when using Dropbox upload)
- odeke-em/drive (when using Google Drive upload)

# Todo
- Fix google drive upload -- "drive push" commands that work manually don't work when run via run_script because something is different about the environment and it doesn't detect that the gdrive managed folder was already initialized.
- Support database dumping on remotes, not just localhost
- Support multiple backup configurations for the same hostname so different data can synced with differing regularity
- Transition some things from shell comands to API calls to reduce runtime environmental dependencies
- Create a daemon to make things more automated
- Add an encryption step to the export