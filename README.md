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