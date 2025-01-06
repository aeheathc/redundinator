# redundinator
Backup program intended for backing up the files of a Linux server, and multiple clients of any platform. Most of the heavy lifting is done with command line calls to common Linux utilities.
- Syncs everything to a central backup store with rsync
- Exports stored backups to a compressed, size-split format suitable for cloud upload (tar+zstd|split)
- Uploads to a selection of cloud providers

If backing up Windows clients with Redundinator, I recommend using backuppc/cygwin-rsyncd.
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
- Transition more things from shell commands to API calls to reduce runtime environmental dependencies and make it less linux-centric
- Provide all cli functionality in web interface
- Keep track of previous exports and manage redownloading from cloud providers
- Add an optional encryption step to the export
- better rsync error handling, ignore routine errors
- Create client apps for data transfer using rsync library instead of relying on rsync daemon especially for android and windows
    - Support database dumping on remotes, not just localhost
    - File recall from remotes
- Support specifying multiple hostnames/IPs for one source as fallbacks, for example, when a client might be connected with any one of multiple network interfaces
- Finish setting up client2 and client3 in Docker config for testing
- Add support for sftp upload of exports
- Remove unsafe rust related to pkcecode in dropbox sdk once a new crate version is published that includes my change making this field pub
- Automatically deal with "temporary but not transient" issues such as Google's daily upload traffic limit of 750GB
- Summarize completed/total when bailing from upload

# Cloud provider upload setup

## Dropbox
1. Go to dropbox developer console and get an App Key to put into the redundinator configuration.
2. Run redundinator with auth_dropbox to get a URL with which to perform interactive authentication. It will give you an oauth token.
3. Run it again with auth_dropbox and pass the token in with dropbox_oauth_token to complete authentication.
4. upload_dropbox should work now. If it stops working due to the auth expiring, just do steps 2 and 3 again.

## Google
Requires Google Workspaces (i.e. a business account, formerly GSuite). Won't work with a normal @gmail google account, even if you've bought storage with Google One.
This is because the only fully automated way of uploading uses a gcp service account, which can only give ownership of files to accounts in the same domain in Workspaces. Yes I tried uploading to a shared folder, it doesn't help, only file ownership matters when determining which account's storage is consumed by the file.
However, as of this writing Workspaces is actually cheaper than Google One for the same amount of storage, when using the tier that gets the most pooled storage per user. The caveat is that when you first sign up your pooled storage starts at 10% of what you paid for and the rest is slowly allocated over several months (!!) unless you contact support and make an advance payment on some of your future bills.

Here's the overview of what you need: a KEYFILE for a SERVICE ACCOUNT which has been granted access in DOMAIN-WIDE DELEGATION to impersonate a Workspaces USER which has access to the DIRECTORY in Google Drive where you want the files to be.

Here's the step-by-step:
- Get the keyfile. When using Workspaces this is harder because (1) there are org policies enabled by default that prevent it, and (2) even project owners and super admins don't have access to change it without adding special roles to their account first
    - Go to Google Cloud main console, OUTSIDE the project, you want organization level.
        - There doesn't seem to be a way to escape the project once you're in it, so find a direct link to the main page for the google cloud console where it will let you select org or project level.
    - Click on IAM/PERMISSIONS
    - Edit your user and add Roles: "Organization Policy Administrator" and "Organization Administrator". (Note that Organization Policy Administrator should be visible at org level, if you are at the project level, this role won't be available in the list).
    - Now with those 2 roles, click on "Organization Policies" under IAM & Admin
    - Search for "Disable service account key creation" and you should be able to click on Edit Policy and change it to Not Enforced.
        - There are 2 different rules with the same name but different ID and you have to disable both of them!
    - Now switch into the project (create it now if you haven't already) and disable those same policies at the project level as well
    - Create a service account in your project if you haven't already.
    - Now you should be able to generate a key file for your service account (select JSON format)
    - Save this file where Redundinator can get at it and put the path to it in the Redundinator config under gdrive_service_account_key_file
- Enable the "Google Drive API" Product in the GCP project
- Fill out gdrive_email in the Redundinator config with the email address of the user who will have access to the storage location.
- Have that user create the google drive folder for storing the backups and put the folder ID in the Redundinator config under gdrive_dir_id
    - The folder ID will be at the end of the URL in your address bar when viewing the folder on the web
    - I recommend creating this folder in a Shared Drive so you can share it with your 'main' google account (a free @gmail account), which will be considered outside the organization, assuming you got Workspaces just for the storage.
        - I do not recommend trying to use a Workspaces account as your 'main' google account as many consumer grade google services don't work well with it.
- Go into the Workspaces Admin console and find the Domain-Wide Delegation feature.
    - Grant DWD to your service account. Make sure you refer to the service account by the NUMERIC "Client ID" and not the id that looks like an email address. This is shown when you open the details for the service account in GCP.
    - For some reason it only works if you select ALL the Google Drive scopes here. You can copy the following:
    - https://www.googleapis.com/auth/drive,https://www.googleapis.com/auth/drive.appdata,https://www.googleapis.com/auth/drive.apps.readonly,https://www.googleapis.com/auth/drive.file,https://www.googleapis.com/auth/drive.meet.readonly,https://www.googleapis.com/auth/drive.metadata,https://www.googleapis.com/auth/drive.metadata.readonly,https://www.googleapis.com/auth/drive.photos.readonly,https://www.googleapis.com/auth/drive.readonly,https://www.googleapis.com/auth/drive.scripts

Congrats. It may have been more complex to initially set up than Dropbox but it's simpler to use as uploads should just work forever and won't require interactive steps.
