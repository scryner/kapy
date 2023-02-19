kapy
====

Kapy is a simple utility designed to copy digital camera photos from an SD card to a disk with transformations.
This tool streamlines the process of transferring photos from your camera to your computer, while also providing the ability to make any necessary image transformations during the copying process.

- Convert from JPEG to HEIC
- Merge GPS information into EXIF from .gpx files on Google Drive
- Adjust image size and compression ratio based on EXIF rate information

## Usage
```shell
$ kapy clone -c ~/.kapy.yaml --from /Volumes/Untitled/DCIM/108HASBL --to ~/images
```

## Disclaimer
To access Google Drive API using your own Google OAuth 2.0 client_id and client_secret, you will need to set up a project on the Google Developers Console and create OAuth 2.0 credentials.
Once you have obtained your credentials, you can set the CLIENT_ID and CLIENT_SECRET as environment variables or include them directly in your code.

```shell
$ CLIENT_ID={YOUR_CLIENT_ID} CLIENT_SECRET={YOUR_SECRET} kapy login
$ kapy clone
```

If you encounter login issues, you can log in again as follows.

```shell
$ kapy clean
$ CLIENT_ID={YOUR_CLIENT_ID} CLIENT_SECRET={YOUR_SECRET} kapy login
```

Or, you can assign CLIENT_ID and CLIENT_SECRET values at compile time.

```shell
$ CLIENT_ID={YOUR_CLIENT_ID} CLIENT_SECRET={YOUR_SECRET} cargo install kapy

OR

$ CLIENT_ID={YOUR_CLIENT_ID} CLIENT_SECRET={YOUR_SECRET} cargo build
```

The Google Drive API has a strict authorization process since it accesses users' sensitive information. This application was originally created for my personal use, and it is difficult to comply with the strict authorization process. If necessary, you should refer to the following document to generate Google OAuth 2.0 credentials:

https://developers.google.com/identity/protocols/oauth2/native-app

The following API scopes must be specified:

* https://www.googleapis.com/auth/drive.metadata.readonly: See information about your Google Drive files.
* https://www.googleapis.com/auth/drive.readonly: See and download all your Google Drive files.


### Configurations
* An example
```yaml
default_path:
  from: /Volumes/Untitled/DCIM/108HASBL 
  to: ~/images
polices:
- rate: [5]
  commands:
   resize: 100%  # default value; ignore it
   format: preserve  # default value
- rate: [4]
  commands:
    quality: 95%
    format: heic
- rate: [1,2,3]
  commands:
    resize: 50% # resize image to 50%
    quality: 95%
    format: heic
- rate: [0]
  commands:
    resize: 36m # resize image to 36m pixels
    quality: 92%
    format: heic
```