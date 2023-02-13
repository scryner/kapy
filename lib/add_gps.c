#include <stdio.h>
#include <stdlib.h>
#include <gexiv2/gexiv2.h>
#include "stream.h"

size_t native_add_gps_info(unsigned char *blob, size_t blob_len, unsigned char **out_blob, double lat, double lon, double alt) {
    GExiv2Metadata *meta = gexiv2_metadata_new();
    GError *error = NULL;
    unsigned char *new_blob = NULL;
    size_t new_len = 0;
    ManagedStreamCallbacks *stream = NULL;

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
