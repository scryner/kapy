#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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

 char** native_get_tags_from_path(const char *path, char **tags, size_t tag_len) {
    GError *error = NULL;
    char **vals = NULL;
    char *tag = NULL;
    char *val = NULL;
    const gchar *mime_type = NULL;
    char *copied_mime = NULL;
    size_t i = 0;

    GExiv2Metadata *meta = gexiv2_metadata_new();

    do {
       // read meta from path
        gexiv2_metadata_open_path(meta, path, &error);
        if (error != NULL) {
            fprintf(stderr, "Failed to read metadata: %s\n", error->message);
            break;
        }

        vals = (char**) malloc(sizeof(char*) * (tag_len+1));
        memset(vals, 0, sizeof(char*) * (tag_len+1));

        // get mime type
        mime_type = (const char*) gexiv2_metadata_get_mime_type(meta);
        if (mime_type != NULL) {
            copied_mime = (char*) malloc(sizeof(char) * strlen(mime_type));
            strncpy(copied_mime, mime_type, strlen(mime_type));
            copied_mime[strlen(mime_type)] = '\0';
            vals[tag_len] = copied_mime;
        }

        // get tags
        for (i = 0; i < tag_len; i++) {
            // try to get tag
            tag = tags[i];
            val = (char*) gexiv2_metadata_try_get_tag_string(meta, tag, &error);
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
