#include <stdio.h>
#include <stdlib.h>
#include <gexiv2/gexiv2.h>
#include "stream.h"

size_t native_add_gps_info_to_blob(unsigned char *blob, size_t blob_len, unsigned char **out_blob, double lat, double lon, double alt) {
    GError *error = NULL;
    unsigned char *new_blob = NULL;
    size_t new_len = 0;
    ManagedStreamCallbacks *stream = NULL;

    GExiv2Metadata *meta = gexiv2_metadata_new();

    do {
        // read meta from blob
        gexiv2_metadata_open_buf(meta, blob, blob_len, &error);
        if (error != NULL) {
            fprintf(stderr, "Failed to read metadata: %s\n", error->message);
            break;
        }

        // insert gps info
        gexiv2_metadata_try_set_gps_info(meta, lon, lat, alt, &error);
        if (error != NULL) {
            fprintf(stderr, "Failed to set gps info: %s\n", error->message);
            break;
        }

        // save to stream
        stream = buf_stream_new(blob, blob_len);

        gexiv2_metadata_save_stream(meta, stream, &error);
        if (error != NULL) {
            fprintf(stderr, "Failed to save meta to stream: %s\n", error->message);
            break;
        }

        new_blob = buf_stream_get_data(stream, &new_len);
        *out_blob = new_blob;

    } while(0);

    // cleaning
    if (error != NULL) {
        g_error_free(error);
    }

    buf_stream_free(&stream);
    g_object_unref(meta);

    return new_len;
}

int native_get_rating_from_path(const char *path) {
    GExiv2Metadata *meta = NULL;
    GError *error = NULL;
    gchar *rating = NULL;
    int result = -1;

    meta = gexiv2_metadata_new();

    do {
        // read meta from path
        gexiv2_metadata_open_path(meta, path, &error);
        if (error != NULL) {
            fprintf(stderr, "Failed to read metadata: %s\n", error->message);
            break;
        }

        // try to read Xmp.xmp.Rating tag
        rating = gexiv2_metadata_try_get_tag_string(meta, "Xmp.xmp.Rating", &error);
        if (error != NULL) {
            fprintf(stderr, "Failed to read tag rating : %s\n", error->message);
            break;
        }

        if (rating != NULL) {
            result = atoi(rating);
        }
    } while(0);

    // clearning
    if (error != NULL) {
        g_error_free(error);
    }

    if (rating != NULL) {
        g_free(rating);
    }

    g_object_unref(meta);

    return result;
}

 unsigned char** native_get_tags_from_path(const char *path, unsigned char **tags, size_t tag_len) {
    GError *error = NULL;
    unsigned char **vals = NULL;
    unsigned char *tag = NULL;
    unsigned char *val = NULL;
    size_t i = 0;

    GExiv2Metadata *meta = gexiv2_metadata_new();

    do {
       // read meta from path
        gexiv2_metadata_open_path(meta, path, &error);
        if (error != NULL) {
            fprintf(stderr, "Failed to read metadata: %s\n", error->message);
            break;
        }

        vals = (unsigned char**) malloc(sizeof(unsigned char*) * tag_len);
        memset(vals, 0, sizeof(unsigned char*) * tag_len);

        for (i = 0; i < tag_len; i++) {
            // try to get tag
            tag = tags[i];
            val = (unsigned char*) gexiv2_metadata_try_get_tag_string(meta, (char*)tag, &error);
            if (error != NULL) {
                fprintf(stderr, "Failed to read tag rating : %s\n", error->message);
                break;
            }

            // add to result
            if (val != NULL) {
                vals[i] = val;
            }
        }
    } while(0);

    // cleaning
    if (error != NULL) {
        g_error_free(error);
    }

    g_object_unref(meta);

    return vals;
 }
