#include <iostream>
#include <stdio.h>
#include <math.h>

#include <exiv2/exiv2.hpp>
#include <exiv2/basicio.hpp>

#include "exif.h"

#define EXIF_KEY_GPS_VERSION    "Exif.GPSInfo.GPSVersionID"
#define EXIF_KEY_GPS_FORMAT     "Exif.GPSInfo.GPSMapDatum"
#define EXIF_KEY_GPS_ALT_REF    "Exif.GPSInfo.GPSAltitudeRef"
#define EXIF_KEY_GPS_ALT        "Exif.GPSInfo.GPSAltitude"
#define EXIF_KEY_GPS_LAT_REF    "Exif.GPSInfo.GPSLatitudeRef"
#define EXIF_KEY_GPS_LAT        "Exif.GPSInfo.GPSLatitude"
#define EXIF_KEY_GPS_LON_REF    "Exif.GPSInfo.GPSLongitudeRef"
#define EXIF_KEY_GPS_LON        "Exif.GPSInfo.GPSLongitude"

// private struct for exif_metadata_t
struct _exif_metadata_private_t {
    Exiv2::Image::UniquePtr image;
};

// internal functions
char* s_to_cstr(std::string &str);
int s_try_destroy_gps_info(exif_metadata_t *self);
int s_try_update_gps_info(exif_metadata_t *self, double lat, double lon, double alt);

exif_metadata_t* exif_metadata_new() {
    exif_metadata_t *self = (exif_metadata_t*) malloc(sizeof(exif_metadata_t));
    memset(self, 0, sizeof(exif_metadata_t));

    exif_metadata_private_t *priv = (exif_metadata_private_t*) malloc(sizeof(exif_metadata_private_t));
    memset(priv, 0, sizeof(exif_metadata_private_t));

    self->priv = priv;
    return self;
}

int exif_metadata_open(exif_metadata_t *self, const char *path) {
    try {
        // read image from file
        self->priv->image = Exiv2::ImageFactory::open(path);

    } catch (Exiv2::Error &e) {
        std::cerr << "Error while read image: " << e << std::endl;
        return -1;
    }

    return 0;
}

int exif_metadata_open_blob(exif_metadata_t *self, const unsigned char *blob, size_t blob_len) {
    try {
        // read image from blob
        self->priv->image = Exiv2::ImageFactory::open(blob, blob_len);

    } catch (Exiv2::Error &e) {
        std::cerr << "Error while read image from blob: " << e << std::endl;
        return -1;
    }

    return 0;
}

char* exif_get_tag_string(exif_metadata_t *self, const char *tag) {
    if (self == nullptr|| self->priv == nullptr) {
        return nullptr;
    }

    if (self->priv->image.get() == nullptr) {
        return nullptr;
    }

    try {
        // read metadata
        self->priv->image->readMetadata();

        if (strncmp("Xmp.", tag, 4) == 0) {
            Exiv2::XmpData &xmpData = self->priv->image->xmpData();
            if (xmpData.empty()) {
                return nullptr;
            }

            std::string val = xmpData[tag].toString();
            return s_to_cstr(val);

        } else {
            Exiv2::ExifData &exifData = self->priv->image->exifData();
            if (exifData.empty()) {
                return nullptr;
            }

            std::string val = exifData[tag].toString();
            return s_to_cstr(val);
        }
    } catch ( ... ) {
        return nullptr;
    }

    return nullptr;
}

char* exif_get_mime(exif_metadata_t *self) {
    if (self == nullptr|| self->priv == nullptr) {
        return nullptr;
    }

    if (self->priv->image.get() == nullptr) {
        return nullptr;
    }

    std::string mime = self->priv->image->mimeType();
    return s_to_cstr(mime);
}

void exif_metadata_destroy(exif_metadata_t **self) {
    if (self == nullptr || *self == nullptr || (*self)->priv == nullptr) {
        return;
    }

    if ((*self)->priv->image.get() != nullptr) {
        (*self)->priv->image.reset();
    }

    free((*self)->priv);
    free(*self);

    *self = NULL;
}


size_t exif_metadata_save_blob(exif_metadata_t *self, unsigned char* blob, size_t blob_len, unsigned char **out_blob) {
    size_t out_blob_len = 0;

    if (self == nullptr || self->priv == nullptr) {
        return 0;
    }

    if (self->priv->image.get() == nullptr) {
        return 0;
    }

    unsigned char *buf = nullptr;

    try {
        // create MemIO class from blob
        Exiv2::MemIo *mem = new Exiv2::MemIo(blob, blob_len);
        Exiv2::BasicIo::UniquePtr memBlock(std::move(mem));

        // make image from blob and read its metadata
        Exiv2::Image::UniquePtr image = Exiv2::ImageFactory::open(std::move(memBlock));
        image->readMetadata();

        // write metadata from self
        Exiv2::AccessMode mode = image->checkMode(Exiv2::mdExif);
        if (mode == Exiv2::amWrite || mode == Exiv2::amReadWrite) {
            image->setExifData(self->priv->image->exifData());
        }

        mode = image->checkMode(Exiv2::mdXmp);
        if (mode == Exiv2::amWrite || mode == Exiv2::amReadWrite) {
            image->setXmpData(self->priv->image->xmpData());
        }

        mode = image->checkMode(Exiv2::mdIptc);
        if (mode == Exiv2::amWrite || mode == Exiv2::amReadWrite) {
            image->setIptcData(self->priv->image->iptcData());
        }

        mode = image->checkMode(Exiv2::mdComment);
        if (mode == Exiv2::amWrite || mode == Exiv2::amReadWrite) {
            image->setComment(self->priv->image->comment());
        }

        image->writeMetadata();

        // copying MemIO memory block to new blob
        mem->seek(0, Exiv2::BasicIo::beg);

        size_t block_len = mem->size();
        if (block_len < 1) {
            return 0;
        }

        buf = (unsigned char*) malloc(sizeof(unsigned char) * block_len);
        size_t read_len = mem->read(buf, block_len);

        out_blob_len = read_len;
        *out_blob = buf;

    } catch (Exiv2::Error &e) {
        std::cerr << "Failed to metadata to blob: " << e << std::endl;
        if (out_blob != nullptr) {
            free(out_blob);
        }
    }

    return out_blob_len;
}

int exif_metadata_add_gps_info(exif_metadata_t *self, double lat, double lon, double alt) {
    if (self == nullptr || self->priv == nullptr || self->priv->image.get() == nullptr) {
        return -1;
    }

    // read metadata
    self->priv->image->readMetadata();

    // try to delete previous gps info
    int rc = s_try_destroy_gps_info(self);
    if (rc != 0) {
        return rc;
    }

    // update gps info
    return s_try_update_gps_info(self, lat, lon, alt);
}

char* s_to_cstr(std::string &str) {
    char *ret = new char[str.length() + 1];
    std::strcpy(ret, str.c_str());

    return ret;
}

int s_try_destroy_gps_info(exif_metadata_t *self) {
    try {
        Exiv2::ExifData &exif_data = self->priv->image->exifData();

        Exiv2::ExifData::iterator exif_iter = exif_data.begin();
        while (exif_iter != exif_data.end()) {
            if (exif_iter->groupName() == "GPSInfo") {
                exif_iter = exif_data.erase(exif_iter);
            } else {
                exif_iter++;
            }
        }
    } catch (Exiv2::Error &e) {
        std::cerr << "Failed to destroy gps info in exif: " << e << std::endl;
        return -1;
    }

    try {
        Exiv2::XmpData &xmp_data = self->priv->image->xmpData();

        Exiv2::XmpData::iterator xmp_iter = xmp_data.begin();
        while (xmp_iter != xmp_data.end()) {
            if (xmp_iter->tagName().compare(0, 3, "GPS") == 0) {
                xmp_iter = xmp_data.erase(xmp_iter);
            } else {
                xmp_iter++;
            }
        }
    } catch (Exiv2::Error &e) {
        std::cerr << "Failed to destroy gps info in xmp: " << e << std::endl;
        return -1;
    }

    return 0;
}

int s_try_update_gps_info(exif_metadata_t *self, double lat, double lon, double alt) {
    try {
        Exiv2::ExifData &exif_data = self->priv->image->exifData();

        // set GPS info version
        Exiv2::ExifKey key(EXIF_KEY_GPS_VERSION);
        Exiv2::ExifData::iterator it = exif_data.findKey(key);
        if (it == exif_data.end()) {
            exif_data [EXIF_KEY_GPS_VERSION] = "2 0 0 0";
        }

        // set GPS info format
        exif_data[EXIF_KEY_GPS_FORMAT] = "WGS-84";

        // set altitude
        if (alt < 0.0) {
            exif_data[EXIF_KEY_GPS_ALT_REF] = "1";
        } else {
            exif_data[EXIF_KEY_GPS_ALT_REF] = "0";
        }

        Exiv2::Rational frac = Exiv2::floatToRationalCast(static_cast<float>(fabs(alt)));
        exif_data[EXIF_KEY_GPS_ALT] = frac;

        // set latitude
        if (lat < 0.0) {
            exif_data[EXIF_KEY_GPS_LAT_REF] = "S";
        } else {
            exif_data[EXIF_KEY_GPS_LAT_REF] = "N";
        }

        double whole;
        double remainder = modf(fabs(lat), &whole);
        int deg = (int) floor(whole);

        const int denom = 1000000;
        remainder = modf(fabs(remainder * 60), &whole);
        int min = (int) floor(whole);
        int sec = (int) floor(remainder * 60 * denom);

        char buf[100];
        snprintf(buf, 100, "%d/1 %d/1 %d/%d", deg, min, sec, denom);
        exif_data[EXIF_KEY_GPS_LAT] = buf;

        // set longitude
        if (lon < 0.0) {
            exif_data[EXIF_KEY_GPS_LON_REF] = "W";
        } else {
            exif_data[EXIF_KEY_GPS_LON_REF] = "E";
        }

        remainder = modf(fabs(lon), &whole);
        deg = (int) floor(whole);

        remainder = modf(fabs(remainder * 60), &whole);
        min = (int) floor(whole);
        sec = (int) floor(remainder * 60 * denom);

        snprintf(buf, 100, "%d/1 %d/1 %d/%d", deg, min, sec, denom);
        exif_data[EXIF_KEY_GPS_LON] = buf;

    } catch (Exiv2::Error &e) {
        std::cerr << "Failed to update gps info in exif: " << e << std::endl;
        return -1;
    }

    return 0;
}
